# 播放卡在 PAUSED 状态排查与修复记录

## 问题描述

TUI 播放器曾出现两类看起来相同的现象：

1. 快速连续切歌后，界面显示 `PAUSED`，按暂停键无效，继续切歌偶尔失效。
2. 某些特定歌曲始终无法播放，例如 `飞跃经济舱 (LIVE版)`，界面停在 `PAUSED`。

这两类问题的 UI 表现相同，但根因不同。最终结论是：不能只看 `PlayerPage.is_playing` 或 rodio 的 pause 状态，必须沿着 `PageAction -> PlayQueue -> do_play_song -> pending_play -> NetunePlayer -> poll_pending_play -> PlayerPage` 完整追踪。

## 最终结论

### 1. 原来的解决方法是否正确

原文中的几轮修复并不是完全错误，但它们只覆盖了部分风险：

| 修复方向 | 是否正确 | 说明 |
|----------|----------|------|
| `generation` 计数器 | 正确，但不完整 | 可以防止旧播放任务晚写入 `PlaybackState`，避免幽灵播放。 |
| `spawn_blocking` 承载音频解码/设备打开 | 正确，但不解决所有取消问题 | `spawn_blocking` 仍不能被 `abort()` 强制停止，所以仍需要 generation 检查。 |
| 音频设备打开重试 | 合理的防御性措施 | 快速切歌时设备可能短暂不可用，但它不是“特定歌曲 PAUSED”的根因。 |
| `TogglePause` recovery | 只能兜底 | `state=None` 时可尝试重播，但如果歌曲本身没有可播放 URL，重播仍会失败。 |
| 封面解码移到 blocking 线程 | 正确 | 避免 debug 模式下图片解码阻塞 tokio async worker。 |

原文的问题是把“PAUSED”过早归因到状态机丢失。后续验证表明，`PAUSED` 也可能是播放失败被静默吞掉后的正常 UI 同步结果。

### 2. 当前确认的根因

#### 根因 A：不可播放歌曲没有被显式建模

网易云 `song_url` 对无版权、VIP、地区限制、下架或现场版资源缺失歌曲可能返回：

```json
{
  "code": 200,
  "data": [
    {
      "id": 123,
      "url": null,
      "br": null,
      "fee": 1
    }
  ]
}
```

`netune-api::song_url()` 会把 `url: null` 转成 `no url available`。此前 `do_play_song()` 的后台任务只是写日志，然后继续获取歌词/封面并返回。`poll_pending_play()` 看到任务成功结束后清掉 loading，`sync_player_state()` 又读到 `duration=0`、`is_playing=false`，于是 UI 显示 `PAUSED`。

这不是暂停，而是“播放不可用”。

#### 根因 B：队列加载后直接调用 `do_play_next()`

`PageAction::PlayQueue` 和歌单详情加载原本是：

```rust
self.play_queue.load(songs);
self.do_play_next().await;
```

`load()` 已经把 current 指向第一首，随后 `do_play_next()` 又调用 `advance()`，导致实际从第二首开始播。连续切歌时更容易过早到达队尾，表现为“不能继续切歌”。

修复后改为播放当前歌曲：

```rust
self.play_queue.load(songs);
if let Some(song) = self.play_queue.current().cloned() {
    self.do_play_song(song).await;
}
```

#### 根因 C：shuffle 预取索引污染新队列

`peek_next()` 在 Shuffle 模式下会缓存 `next_shuffle_idx`，保证 UI 预取的下一首和 `advance()` 实际播放的下一首一致。此前在 `load()`、`jump()`、`skip_to()`、`prev()`、`shuffle()`、切换播放模式等状态变化后没有清空该缓存。

如果旧队列缓存了一个索引，新队列更短，下一次 `advance()` 可能返回 `None`。App 已经 stop 了当前播放器，但没有下一首可播，UI 就停在 `PAUSED`。

## 当前修复方案

### 1. 播放失败显式传回 App

`PendingPlayResult` 增加 `playback_error`：

```rust
struct PendingPlayResult {
    song_id: u64,
    _song: Song,
    playback_error: Option<String>,
    audio_bytes: Option<Vec<u8>>,
    lyrics: Option<Lyrics>,
    cover_protocol: Option<ratatui_image::protocol::Protocol>,
}
```

以下失败都会写入 `playback_error`：

- `song_url()` 返回 `no url available`
- 下载音频失败
- 读取音频 body 失败
- `play_from_bytes()` 解码或打开设备失败
- 没有音频播放器实例

### 2. 播放失败时跳过或显示错误

`poll_pending_play()` 处理当前歌曲失败结果：

```rust
if let Some(error) = result.playback_error {
    self.set_player_loading(false);
    let should_try_next = self
        .play_queue
        .peek_next()
        .is_some_and(|song| song.id != result.song_id);
    if should_try_next {
        self.do_play_next().await;
    } else {
        pp.set_playback_error(error);
    }
    return;
}
```

效果：

- 队列播放时，当前歌曲不可播放会自动尝试下一首。
- 单曲播放或队尾无下一首时，播放器页歌词区域显示 `Playback unavailable: ...`。
- 不再把播放失败伪装成普通 `PAUSED`。

### 3. 队列状态变更时清空 shuffle 预取

以下操作都会清空 `next_shuffle_idx`：

- `load`
- `remove`
- `jump`
- `skip_to`
- `prev`
- `set_repeat_mode`
- `cycle_mode`
- `shuffle`

原则：任何会改变歌曲列表、当前索引或播放模式的操作，都必须让 shuffle 预取失效。

## 固化后的 TUI 播放调试流程

### 1. 复现

启动 TUI：

```sh
RUST_LOG=debug cargo run --release
```

日志固定写入：

```sh
/tmp/netune.log
```

另开终端观察：

```sh
tail -f /tmp/netune.log
```

复现步骤：

1. 搜索目标歌曲。
2. 播放可疑歌曲，例如 `飞跃经济舱 (LIVE版)`。
3. 观察是否出现 `Playback failed for current song`。
4. 如果是队列播放，确认是否自动跳到下一首。
5. 如果没有下一首，确认播放器页是否显示 `Playback unavailable: ...`。

### 2. 必查日志点

按顺序看这些日志，而不是只看 UI：

| 边界 | 期望日志/状态 | 异常含义 |
|------|----------------|----------|
| `song_url` | 有 URL 或 `Failed to get song URL` | 无版权、VIP、下架、地区限制或接口失败。 |
| 音频下载 | 成功读到 bytes 或 `Failed to download audio` | CDN、网络或 URL 失效。 |
| `play_from_bytes` | `Playback started from cached bytes` | 解码或设备打开失败。 |
| `poll_pending_play` | `poll_pending_play processed` | 正常完成并更新 UI。 |
| 失败分支 | `Playback failed for current song` | 当前歌曲不可播放，必须跳过或显示错误。 |

### 3. 判断准则

不要直接把 `PAUSED` 当成暂停状态。先分三类：

1. `duration > 0` 且 `is_paused=true`：真实暂停。
2. `duration == 0` 且 `pending_play.is_some()`：仍在加载。
3. `duration == 0` 且 `pending_play.is_none()`：播放失败或没有 active state，需要看 `playback_error` 和日志。

### 4. 回归测试

本问题固定用两个测试防回归：

```sh
cargo test -p netune-player test_queue_load_clears_stale_shuffle_peek
cargo test -p netune-tui failed_playback_advances_to_next_track
```

完整相关验证：

```sh
cargo test -p netune-player
cargo test -p netune-tui
```

## 经验规则

1. `PAUSED` 是 UI 显示结果，不一定代表用户按了暂停。
2. 播放失败必须进入显式错误状态，不能只写日志后清 loading。
3. `abort()` 不能取消已经运行的 blocking work，跨线程播放仍需要 generation。
4. `peek_next()` 这种缓存必须有清晰失效点。
5. TUI 问题必须沿事件、队列、后台任务、播放器状态、页面状态逐层追踪。
