# 暂停键失灵问题排查记录

## 问题描述

快速连续切换歌曲后，暂停键无法响应，歌曲不播放，UI 卡在 paused 状态。

## 排查过程

### 第一轮：线程竞态条件（部分正确）

**假设**：`play_from_bytes` 内部 spawn 的 `std::thread` 在 tokio task 被 abort 后继续运行，造成"幽灵播放"。

**关键代码**（修复前）：

```rust
// player.rs — play_from_bytes 的旧实现
async fn play_from_bytes(&self, bytes: Vec<u8>) -> Result<()> {
    // 1. 停掉旧播放
    if let Ok(mut state) = self.state.lock() {
        if let Some(old) = state.take() {
            old.player.stop();  // state 变成 None
        }
    }

    // 2. spawn 一个独立线程做音频解码
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::Builder::new()
        .spawn(move || {
            // ... 音频解码、创建设备、播放 ...
            rodio_player.append(decoder);  // 音频开始播放！
            *state = Some(PlaybackState { ... });  // 写入 state
            let _ = tx.send(result);
        })?;

    rx.await?  // 等待线程完成
}
```

```rust
// app.rs — 切歌时的调用链
async fn do_play_song(&mut self, song: Song) {
    if let Some(handle) = self.pending_play.take() {
        handle.abort();  // 取消旧 task
    }
    if let Some(ref player) = self.player {
        player.stop();   // state = None，音频停止
    }
    // spawn 新 task → 调用 play_from_bytes
    self.pending_play = Some(tokio::spawn(async move {
        p.play_from_bytes(bytes).await;  // 内部 spawn 线程
    }));
}
```

**问题分析**：

当用户快速切歌 A → B → C 时：

```
时间线：
t0  do_play_song(A) → stop() [state=None] → spawn task_A
t1  task_A: play_from_bytes(A) → spawn thread_A [正在解码...]
t2  do_play_song(B) → abort task_A → stop() [state=None] → spawn task_B
    ⚠ thread_A 没有被 abort！它还在跑！
t3  task_B: play_from_bytes(B) → spawn thread_B [正在解码...]
t4  do_play_song(C) → abort task_B → stop() [state=None] → spawn task_C
    ⚠ thread_A 和 thread_B 都还在跑！
t5  thread_A 完成 → 写入 state(Some(A)) → 但 C 才是当前歌曲
t6  thread_B 完成 → 写入 state(Some(B)) → 覆盖了 A
t7  thread_C 完成 → 写入 state(Some(C)) → 最终状态
```

关键问题：**`handle.abort()` 只能取消 tokio task，不能取消 `std::thread`**。线程会继续运行并写入过期的 state。

**知识点：tokio task 的取消机制**

```
handle.abort()
    │
    ▼
task 被标记为 "cancelled"
    │
    ▼
task 在下一个 .await 点收到 Cancelled 错误
    │
    ▼
task 的 future 被 drop
    │
    ├── async 代码：.await 被中断，已持有的资源被释放
    │
    ├── spawn_blocking 闭包：不受影响，继续运行直到返回
    │
    └── std::thread：不受影响，继续运行到自然结束
```

**修复**：添加 `AtomicU64` generation counter，线程写入前检查计数器。

```rust
// 修复后的 play_from_bytes
async fn play_from_bytes(&self, bytes: Vec<u8>) -> Result<()> {
    let my_gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
    // ... stop old playback ...

    std::thread::Builder::new().spawn(move || {
        // ... 解码、创建设备、播放 ...
        rodio_player.append(decoder);

        // ⭐ 写入前检查：generation 是否还匹配？
        if gen_ref.load(Ordering::SeqCst) != my_gen {
            return Ok(());  // 已被更新的 play 取代，丢弃结果
        }

        *state = Some(PlaybackState { ... });
        Ok(())
    })?;
}
```

```rust
// stop() 也要递增 generation
fn stop(&self) {
    self.generation.fetch_add(1, Ordering::SeqCst);
    if let Ok(mut state) = self.state.lock() {
        if let Some(ref ps) = *state {
            ps.player.stop();
        }
        *state = None;
    }
}
```

**结果**：解决了幽灵播放，但问题依旧。

---

### 第二轮：spawn_blocking 替代 std::thread

**假设**：独立 `std::thread` 不受 tokio 管理，应该用 `spawn_blocking`。

**修复**：

```rust
// 旧：std::thread + oneshot channel
let (tx, rx) = tokio::sync::oneshot::channel();
std::thread::Builder::new().spawn(move || {
    // ... 重活 ...
    let _ = tx.send(result);
})?;
rx.await.map_err(|e| ...)??

// 新：spawn_blocking
let handle = tokio::task::spawn_blocking(move || {
    // ... 重活（同样的代码）...
    Ok(())
});
handle.await.map_err(|e| ...)??
```

**知识点：tokio 的两种线程池**

| 线程池 | 用途 | 调度方式 | 特点 |
|--------|------|----------|------|
| async worker | `tokio::spawn` 的 future | 协作式，`.await` 点切换 | 数量少（默认=CPU核数） |
| blocking pool | `spawn_blocking` 的闭包 | OS 线程抢占式 | 动态扩展，上限 512 |

```rust
// async worker 上的任务必须在 .await 点让出
tokio::spawn(async {
    network_request().await;  // 让出：等网络 I/O
    another_request().await;  // 让出：等网络 I/O
});

// spawn_blocking 的闭包不需要 .await，OS 线程会抢占调度
tokio::task::spawn_blocking(|| {
    heavy_computation();  // 不让出也没关系，OS 线程会调度
});
```

同时添加了 early generation check——在重活开始前就检查，避免浪费 CPU：

```rust
let handle = tokio::task::spawn_blocking(move || {
    // ⭐ 最早的检查点：解码前
    if gen_ref.load(Ordering::SeqCst) != my_gen {
        return Ok(());
    }

    let decoder = Decoder::new(cursor)?;

    // ⭐ 第二个检查点：开音频设备前（这个操作可能和其他 task 冲突）
    if gen_ref.load(Ordering::SeqCst) != my_gen {
        return Ok(());
    }

    let device_sink = DeviceSinkBuilder::open_default_sink()?;
    rodio_player.append(decoder);

    // ⭐ 最后一个检查点：写入 state 前
    if gen_ref.load(Ordering::SeqCst) != my_gen {
        return Ok(());
    }

    *state = Some(PlaybackState { ... });
    Ok(())
});
```

**结果**：问题依旧。`spawn_blocking` 的任务在 JoinHandle 被 drop 后仍然继续运行。

---

### 第三轮：设备打开重试

**假设**：音频设备竞争导致 `DeviceSinkBuilder::open_default_sink()` 失败。

**修复**：

```rust
// 设备打开重试逻辑
let mut device_sink = None;
for attempt in 1..=3u32 {
    match DeviceSinkBuilder::open_default_sink() {
        Ok(mut ds) => {
            ds.log_on_drop(false);
            device_sink = Some(ds);
            break;
        }
        Err(e) => {
            tracing::warn!(attempt, error = %e, "Failed to open audio device, retrying");
            if attempt < 3 {
                std::thread::sleep(std::time::Duration::from_millis(100));
            } else {
                return Err(NetuneError::Player(format!(
                    "Failed to open audio device after 3 attempts: {e}"
                )));
            }
        }
    }
}
```

**结果**：日志中没有出现设备打开失败。问题依旧。

---

### 第四轮：TogglePause 兜底逻辑

**假设**：state 丢失后用户无法恢复。

**修复**：

```rust
// 旧的 TogglePause 处理
PageAction::TogglePause => {
    if let Some(ref player) = self.player {
        player.toggle_pause();  // state=None 时什么都不做
    }
}

// 新的 TogglePause 处理：三种情况分别处理
PageAction::TogglePause => {
    let action = self.player.as_ref().map(|player| {
        if player.duration() > 0.0 {
            ToggleAction::Toggle           // 正常播放中 → 切换暂停
        } else if self.pending_play.is_some() {
            ToggleAction::Ignore           // 正在加载 → 忽略
        } else {
            ToggleAction::Replay(current_song)  // state 丢失 → 重播
        }
    });

    match action {
        Some(ToggleAction::Toggle) => player.toggle_pause(),
        Some(ToggleAction::Replay(song)) => self.do_play_song(song).await,
        _ => {}
    }
}
```

**知识点：Rust 借用检查器的限制**

这个修复遇到了借用冲突：

```rust
// ❌ 编译失败：不可变借用跨越了可变借用
if let Some(ref player) = self.player {      // 借用 self.player
    if player.duration() > 0.0 {
        player.toggle_pause();
    } else {
        self.do_play_song(song).await;        // 需要 &mut self！冲突！
    }
}
```

解决方法——先做决策（不可变借用），再执行（可变借用）：

```rust
// ✓ 先用不可变借用做决策
let action = self.player.as_ref().map(|player| {
    if player.duration() > 0.0 { ToggleAction::Toggle }
    else { ToggleAction::Replay(song) }
});

// 再用可变借用执行（不再持有不可变借用）
match action {
    Some(ToggleAction::Toggle) => { self.player.as_mut().unwrap().toggle_pause(); }
    Some(ToggleAction::Replay(s)) => { self.do_play_song(s).await; }
    _ => {}
}
```

**结果**：按空格能触发 recovery（显示 loading），但歌曲仍然不播放。

---

### 第五轮：日志分析（找到根因）

请求用户提供 `RUST_LOG=debug` 日志。

**关键日志片段**：

```
08:55:34.236  DEBUG player.rs:220  Playback started from cached bytes duration=304.88
08:55:34.532  INFO  app.rs:364    poll_pending_play processed ms=0
                                    ← 正常：playback 启动，loading 清除

... 用户快速切歌，多次重复上述模式 ...

08:55:34.532  INFO  app.rs:364    poll_pending_play processed ms=0
                                    ← 最后一次成功的 playback
... 8 秒空白 ...
08:55:42.521  INFO  app.rs:281    Cover decoded in background task ms=7367
                                    ← ⚠ 旧 task 的 cover 解码耗时 7.3 秒！
08:55:42.576  INFO  app.rs:364    poll_pending_play processed ms=0
08:55:43.880  WARN  app.rs:1138   TogglePause with no active state, replaying current song
                                    ← 用户按空格，触发 recovery
... 没有 "Playing from audio cache" 日志！play_from_bytes 根本没执行 ...
08:55:50.831  INFO  app.rs:281    Cover decoded in background task ms=19352
                                    ← 又一个旧 task 的 cover 解码，耗时 19 秒！
08:55:51.479  INFO  app.rs:364    poll_pending_play processed ms=0
```

**关键发现**：

1. recovery 触发后没有 "Playing from audio cache" 日志 → `play_from_bytes` **根本没执行到**
2. cover 解码耗时 7-19 秒（debug 模式下 `image::load_from_memory` 是纯 CPU 操作）
3. 多个旧 task 的 cover 解码同时在跑

**根因**：cover 解码在 tokio async worker 上直接运行，**阻塞了整个 runtime**。

**问题代码**（修复前）：

```rust
// app.rs — spawned tokio task 内部
self.pending_play = Some(tokio::spawn(async move {
    // ① 缓存读取（async I/O，正常）
    let (audio_result, lyrics_bytes, cover_bytes) = tokio::join!(
        tokio::fs::read(&audio_path),
        tokio::fs::read(&lyrics_path),
        tokio::fs::read(&cover_path),
    );

    // ② 音频播放（内部用 spawn_blocking，正常）
    p.play_from_bytes(bytes).await;

    // ③ 网络请求获取歌词（async I/O，正常）
    let lyrics = client.lyrics(song_id).await.ok();

    // ④ cover 解码 — ⚠ 这里是问题！
    let cover_protocol = cover.and_then(|bytes| {
        let img = image::load_from_memory(&bytes).ok()?;  // 💥 纯 CPU 操作！
        let protocol = picker.new_protocol(img, size, Resize::Fit(None)).ok();
        // 在 debug 模式下，上面两行耗时 7-19 秒
        // 这 7-19 秒内，tokio async worker 被完全占住
        // 其他所有 async task（包括新的 do_play_song）全部排队等待
        protocol
    });

    PendingPlayResult { song_id, audio_bytes, lyrics, cover_protocol }
}));
```

**知识点：tokio async worker 的饥饿问题**

tokio runtime 默认只有和 CPU 核心数一样多的 async worker 线程（比如 8 核 = 8 个 worker）。每个 worker 在任意时刻只能运行一个 task。task 只有在 `.await` 点才会让出 worker。

```
async worker 线程 (假设只有 1 个，简化示意)

task_1: |████ cover 解码 19秒 ████|.await|返回|
task_2: 　　　　　　　　排队等待中...　　　　|执行|.await|
task_3: 　　　　　　　　排队等待中...　　　　　　|执行|

← task_2 和 task_3 被 task_1 的 CPU 密集操作饿死了
```

快速切歌时，每次切歌 spawn 一个新 task。旧 task 的 cover 解码占住 worker，新 task 无法执行。用户看到的现象就是：loading 转几秒，然后回到暂停状态。

**修复**：把 cover 解码移到 `spawn_blocking`。

```rust
// 修复后的代码
let cover_protocol = match cover {
    Some(bytes) => {
        match tokio::task::spawn_blocking(move || {
            // 现在这些 CPU 密集操作在 blocking 线程池上运行
            // 不会阻塞 async worker
            let img = image::load_from_memory(&bytes).ok()?;
            let protocol = picker.new_protocol(img, size, Resize::Fit(None)).ok();
            protocol
        }).await {
            Ok(protocol) => protocol,
            Err(e) => {
                tracing::warn!(error = %e, "Cover decode task failed");
                None
            }
        }
    }
    None => None,
};
```

**结果**：问题解决。

---

## 涉及的 Commit

| Commit | 内容 | 是否有效 |
|--------|------|----------|
| `4a340c5` | generation counter | ✓ 防止幽灵播放 |
| `26f787f` | spawn_blocking + early gen check | ✓ 更好的线程管理 |
| `1a761f5` | 设备打开重试 | ✓ 防御性措施 |
| `e29fb6c` | TogglePause 兜底逻辑 | ✓ 用户可恢复 |
| `86a2fa7` | cover 解码移到 spawn_blocking | ✓ **根因修复** |

## 教训总结

### 1. 先看日志，再改代码

前四轮都在猜测问题原因，改了四次都没解决。第五轮看了日志后 5 分钟就定位了根因。

**规则**：任何涉及运行时行为的 bug，第一步应该是加日志复现，而不是凭代码走读猜测。

### 2. async worker 上不能跑 CPU 密集操作

tokio 的 async worker 线程是协作式调度。一个 task 不 `.await` 就不会让出 worker。`image::load_from_memory` 耗时 7-19 秒（debug 模式），直接在 async worker 上运行等于把整个 worker 冻结 7-19 秒。

**规则**：async 函数中超过 1ms 的同步 CPU 操作，必须放到 `spawn_blocking`。

### 3. spawn_blocking / std::thread 不能被 abort

`handle.abort()` 只能在 `.await` 点取消 future。`spawn_blocking` 的闭包和 `std::thread` 都不包含 `.await`，所以不会被取消。

**规则**：需要取消能力的长时间操作，必须在操作内部检查取消标志（generation counter）。

### 4. Generation Counter 模式

用于跨线程/task 的"版本号"机制，解决取消和竞态问题：

```rust
// 每次发起新操作时递增
let my_gen = generation.fetch_add(1, Ordering::SeqCst) + 1;

// 在关键节点检查版本是否还匹配
if generation.load(Ordering::SeqCst) != my_gen {
    return; // 已被更新的操作取代，安全退出
}
```

适用于：异步操作链中的取消传播、"只保留最新结果"的去重、多生产者单消费者的结果竞争。

### 5. 问题可能是多层的

这个 bug 实际上是多个问题叠加：
- 线程竞态（generation counter 修复）
- 音频设备竞争（重试机制防御）
- async runtime 阻塞（spawn_blocking 修复）
- 用户无法恢复（TogglePause 兜底）

单独修复任何一层都不够。需要系统性地排查每一层。

---

## 附录：Rust 异步编程速查

### spawn vs spawn_blocking 选择

```
你的代码里有 .await 吗？
├── 有 → tokio::spawn（async task）
└── 没有 → tokio::task::spawn_blocking（blocking task）
```

| 操作 | 类型 | 正确用法 |
|------|------|----------|
| 网络请求 (reqwest) | async I/O | `spawn` + `.await` |
| 文件读写 (tokio::fs) | async I/O | `spawn` + `.await` |
| 图片/音频解码 | CPU 密集 | `spawn_blocking` |
| JSON 序列化（大数据） | CPU 密集 | `spawn_blocking` |
| std::fs::read | 同步阻塞 I/O | `spawn_blocking` |

### 常见陷阱

| 陷阱 | 症状 | 修复 |
|------|------|------|
| async worker 上跑 CPU 密集 | UI 卡顿、task 堆积 | `spawn_blocking` |
| abort 后 spawn_blocking 继续运行 | 资源泄漏、竞态 | generation counter |
| Mutex 跨 .await 持有 | 死锁（tokio 检测并 panic） | 缩小锁范围 |
| 不可变借用跨越可变借用 | 编译错误 | 先决策再执行 |
| `is_paused()` 在曲目结束后返回 false | 状态不准 | 用 `pos >= dur` 判断 |
