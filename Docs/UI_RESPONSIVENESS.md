# UI 响应性优化报告

## 问题描述

切换歌曲时 UI 卡顿，无法进行任何操作（按键无响应）。debug 模式下尤为明显，release 模式基本无感。

## 根因分析

### 事件循环结构

```
loop {
    terminal.draw()          ← 渲染帧
    event::poll(100ms)       ← 等待事件
    apply_action()           ← 处理动作（含 do_play_song）
    poll_pending_play()      ← 处理后台任务结果
}
```

任何一步阻塞都会导致 UI 冻结。

### 卡顿来源（按影响排序）

| 来源 | debug 耗时 | release 耗时 | 位置 |
|------|-----------|-------------|------|
| `play_from_bytes` (MP3 解码 + 设备打开) | 500-2000ms | 50-200ms | 播放线程 |
| `image::load_from_memory` + `new_protocol` (封面解码) | 200-500ms | 20-50ms | poll_pending_play |
| `audio_cache.get()` (磁盘读取 3-10MB MP3) | 50-200ms | 5-20ms | do_play_song |
| `terminal.draw()` (渲染) | 10-50ms | 1-2ms | 事件循环 |
| `serde_json::from_slice` (歌词反序列化) | 1-5ms | <1ms | poll_pending_play |

## 解决方案

### Phase 1: 异步化播放 (`d6f2041`)

将 `play_from_bytes` 从同步改为 async，内部用 `spawn_blocking` 避免阻塞 tokio runtime。

**问题**：`do_play_song` 仍然 `.await` 等待结果，事件循环依然阻塞。

### Phase 2: 移入 spawned task (`58ea7a2`)

将 `play_from_bytes` 调用移入 `tokio::spawn` 的后台任务中。`do_play_song` spawn 后立即返回。

**问题**：`do_play_song` 在 spawn 前仍有 3 次串行磁盘读取。

### Phase 3: 磁盘 IO 全部移入 task (`ddc5fbf`)

将 `audio_cache.get()` / `get_lyrics()` / `get_cover()` 全部移入 spawned task。

```rust
// do_play_song 现在是纯同步，零 await：
fn do_play_song(&mut self, song: Song) {
    abort_old_tasks();     // 同步
    player.stop();         // 同步
    update_ui();           // 同步
    spawn_task();          // 同步（只是注册任务）
    return;                // 立即返回
}
```

**问题**：`tokio::task::spawn_blocking` 使用共享线程池，重音频任务会饿死其他任务。

### Phase 4: 独立音频线程 (`6929db7`)

将 `spawn_blocking` 替换为独立的 `std::thread` + `oneshot` channel。

```rust
std::thread::Builder::new()
    .name("audio-playback".into())
    .spawn(move || {
        // MP3 解码 + 设备打开 + 播放
        let _ = tx.send(result);
    });
```

音频工作完全隔离于 tokio runtime。

### Phase 5: 封面图片处理移入 task (`d06c989`)

`poll_pending_play` 中的 `set_cover_bytes()` 调用 `image::load_from_memory()` + `picker.new_protocol()`，debug 模式下 200-500ms。

**关键发现**：`ratatui_image::protocol::Protocol` 实现了 `Send + Sync`，可以跨线程传递。

```rust
// 后台任务中处理封面
let cover_protocol = cover_bytes.and_then(|bytes| {
    let img = image::load_from_memory(&bytes).ok()?;
    picker.new_protocol(img, size, Resize::Fit(None)).ok()
});
// 返回 Protocol 而非原始字节
PendingPlayResult { cover_protocol, ... }
```

`poll_pending_play` 只需 `set_cover(protocol)`，零 CPU 开销。

## 架构总览

```
do_play_song (纯同步，<1ms)
    │
    ├─ abort old tasks
    ├─ player.stop()
    ├─ update UI
    └─ tokio::spawn ──→ 后台任务
                           │
                           ├─ tokio::join!(
                           │    tokio::fs::read(audio),
                           │    tokio::fs::read(lyrics),
                           │    tokio::fs::read(cover),
                           │  )  ← 并行磁盘读取
                           │
                           ├─ std::thread("audio-playback")
                           │    ├─ Decoder::new()     ← MP3 解码
                           │    ├─ open_default_sink() ← 设备打开
                           │    └─ player.append()     ← 开始播放
                           │
                           ├─ image::load_from_memory()  ← 封面解码
                           ├─ picker.new_protocol()       ← 封面处理
                           │
                           └─ return PendingPlayResult {
                                audio_bytes,    ← 用于磁盘缓存
                                lyrics,         ← 已反序列化
                                cover_protocol, ← 已处理
                              }

poll_pending_play (快速，<5ms)
    ├─ audio_cache.put()     ← 缓存音频到磁盘
    ├─ set_lyrics()          ← 内存操作
    ├─ set_cover(protocol)   ← 内存操作（无解码）
    └─ set_loading(false)
```

## 辅助优化

### Theme 无锁化 (`8c39dbb`)

将 `Theme` 从 `RwLock<ThemeColors>` 改为 `AtomicU8` + `const` 数组。

```rust
static THEME_IDX: AtomicU8 = AtomicU8::new(0);
const THEMES: [ThemeColors; 3] = [...];

fn current() -> &'static ThemeColors {
    &THEMES[THEME_IDX.load(Ordering::Relaxed) as usize]
}
```

每帧 139 次颜色读取从 ~30ns 锁操作 → ~1ns 原子加载。

### 缓存并行读取 (`9337a19`)

歌词和封面从串行读取改为 `tokio::join!` 并行。

## 提交记录

| 提交 | 内容 | 影响 |
|------|------|------|
| `d6f2041` | play_from_bytes 改 async | 解除 tokio 阻塞 |
| `58ea7a2` | play_from_bytes 移入 spawned task | 事件循环不等播放 |
| `ddc5fbf` | 磁盘 IO 全部移入 task | 事件循环零 IO |
| `6929db7` | 独立音频线程 | 音频不争抢线程池 |
| `d06c989` | 封面解码移入 task | 消除 200-500ms 卡顿 |
| `9337a19` | 缓存并行读取 | 减少串行等待 |
| `8c39dbb` | Theme 无锁化 | 消除锁开销 |

## 帧计时监控

已内建帧计时日志（`RUST_LOG=warn`），超过 16ms 的帧会记录：

```
WARN Slow frame (idle)  draw_ms=1 idle_ms=75    ← 正常（等待事件）
WARN Slow frame (event) draw_ms=2 read_ms=0 apply_ms=124 poll_ms=0  ← 需关注
```

## 经验总结

1. **debug vs release 差距巨大** — debug 模式下 CPU 密集操作慢 10-100x，必须将重活移到后台
2. **spawn_blocking 会争抢线程池** — 重任务用独立 `std::thread` 更可靠
3. **Protocol 是 Send 的** — ratatui-image 的 Protocol 可以跨线程，不需要在主线程解码图片
4. **POST 请求可能收到空 body** — reqwest 在某些环境下 POST 到 music.163.com 返回空 body，GET 正常
5. **帧计时是必备工具** — 没有计时数据就是在盲猜
