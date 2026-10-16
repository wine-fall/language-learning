# language-learning

A lightweight terminal app for learning English by listening: play an audio
episode with its subtitle file, study it scene by scene and sentence by
sentence, and review the expressions you saved with spaced repetition.

No audio or subtitle content ships with this repo — bring your own files.
Everything under `fixtures/` is made-up dialogue written for tests.

下面用中文说明。

## 安装

### Homebrew（macOS，推荐）

```sh
brew tap wine-fall/language-learning https://github.com/wine-fall/language-learning
brew install listen
```

以后更新用 `brew upgrade listen`，卸载用 `brew uninstall listen`。

### 从源码构建

需要 Rust（stable，1.88 以上）。没有的话：

```sh
curl https://sh.rustup.rs -sSf | sh
```

#### macOS

```sh
cargo build --release
./target/release/listen --help
# 可选：装到 ~/.cargo/bin
cargo install --path .
```

macOS 只需要 Xcode Command Line Tools（`xcode-select --install`，rustup 本身也需要），音频走 CoreAudio，不用装别的库。

#### Linux

真实音频播放依赖 ALSA 开发包（Debian/Ubuntu 上是 `libasound2-dev` 和
`pkg-config`）。没有声卡或不想装时，可以构建不出声的版本，其余功能完全一样，
播放进度按时钟走：

```sh
cargo build --release --no-default-features
```

## 准备文件

同一目录下放同名文件：

```
episode.m4a           音频（m4a/AAC）
episode.srt           英文字幕（必需）
episode.words.json    重点表达（可选，见下文）
```

`words.json` 可以手写，也可以用 Claude Code 生成：

```sh
LISTEN=./target/release/listen scripts/annotate.sh path/to/episode.srt
```

脚本用 `listen cues` 给句子编号，把编号后的字幕和
`scripts/annotate-prompt.md` 里的提示词交给 `claude -p`，再用 `listen check`
校验结果后写出 `episode.words.json`。每句最多 3 个表达，优先口语短语和超出
3000 常用词的词。已存在的输出不会被覆盖（`FORCE=1` 可强制），想换模型可以设
`CLAUDE_ARGS="--model sonnet"`。

## 用法

```sh
listen play path/to/episode.m4a     # 学习一集
listen review                       # 复习今天到期的卡片
listen review --all                 # 复习全部卡片（不管是否到期）
listen cues path/to/episode.srt     # 打印句子编号（words.json 的 cue 用它）
listen check path/to/episode.srt path/to/episode.words.json   # 校验 words.json
```

再次打开同一集会从上次所在的场景继续。

## 学习流程

每集按场景学习，每个场景依次经过五个阶段（界面顶部的 ①-⑤）：

1. **① 预习**：列出本场景 5-8 个重点表达和中文释义（来自 words.json）。
2. **② 盲听**：不显示字幕，连续播放整场。
3. **③ 逐句精听**：每句播完自动暂停。可以按 `d` 听写，回车后显示英文原句并逐词对比
   （原句里漏听的词标红；你的听写里听错的词标红并划掉，原句里没有、多打的词用紫色划掉，
   并单独列出；比较时忽略大小写和标点，`it's` 和 `it is`、`don't` 和 `do not` 这类缩写
   视为相同）。然后按 `y` 标“听懂”或 `m` 标“没听清”，自动进入下一句。
4. **④ 复听**：只回放标为“没听清”的句子。
5. **⑤ 无字幕连播**：最后再无字幕把整场连播一遍。

**复习**是独立的 `listen review` 模式：卡片先播放原剧里那一句的音频，回想后按 Enter
翻面看表达、释义和原句，再按 `y`/`n` 自评。

字幕只有两档：关 / 英文全文。中文释义只出现在预习页和复习卡上，不显示整句翻译。

### 场景切分

- words.json 里有 `scenes` 时以它为准。
- 否则按字幕间隔切：两句之间空白 ≥ 4 秒视为换场，同时限制每场 20-50 句
  （太早出现的间隔会被忽略；50 句内都没有长间隔时，在允许范围内最大的间隔处切）。
  少于 20 句的整集算一个场景。

## 按键

学习模式（`listen play`）：

| 键 | 作用 |
| --- | --- |
| `Enter` | 进入下一阶段：预习 → 盲听 → 逐句精听 → 复听 → 无字幕连播 → 下一场景 |
| `space` | 暂停 / 继续；精听时一句播完后按它播下一句 |
| `j` / `k` | 下一句 / 上一句（方向键也可以） |
| `r` | 重播当前句（盲听/连播时从场景开头重放） |
| `p` | 逐句自动暂停 开/关 |
| `c` | 字幕 关 / 英文全文（盲听和最后连播时始终隐藏） |
| `d` | 听写当前句：输入后 Enter 提交，Tab 重播，Esc 取消 |
| `y` / `m` | 标记 听懂 / 没听清，并进入下一句 |
| `1`-`9` | 收藏当前句的第 N 个重点表达（预习页上是列表里的第 N 个），再按一次取消 |
| `[` / `]` | 字幕时间轴整体 -0.25 / +0.25 秒，按文件保存 |
| `<` / `>` | 上一场景 / 下一场景 |
| `?` | 帮助 |
| `q` | 保存并退出 |

复习模式（`listen review`）：

| 键 | 作用 |
| --- | --- |
| `space` | 播放 / 重播原句 |
| `Enter` | 翻面 |
| `y` / `n` | 记得 / 忘了 |
| `?` | 帮助 |
| `q` | 退出 |

复习用简单的 Leitner 分档：5 档，间隔 1 / 2 / 4 / 7 / 15 天。记得升一档，忘了回第一档。
新收藏的卡片在第一档，第二天到期。

## words.json

```json
{
  "items": [
    { "cue": 2, "text": "grab a seat", "meaning": "坐吧；找个位子坐", "note": "口语" },
    { "cue": 5, "text": "chewed me out", "meaning": "狠狠训斥了我", "note": "原形 chew someone out" }
  ],
  "scenes": [
    { "start": 1, "title": "At the café" },
    { "start": 25 }
  ]
}
```

`items`（必填，可为空数组）每项：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `cue` | 整数 | 所在句子的序号，从 1 开始，即 `listen cues` 打印的编号 |
| `text` | 字符串 | 表达原文，按句中实际写法（便于在原句中高亮） |
| `meaning` | 字符串 | 中文释义 |
| `note` | 字符串，可选 | 备注，如原形、语气 |

同一句的多个表达按文件中的顺序对应数字键 1-9。预习页每场最多显示 8 个：
超过时优先多词短语，其次按句子顺序。

`scenes`（可选）每项：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `start` | 整数 | 场景第一句的序号；场景一直延续到下一个场景开始 |
| `title` | 字符串，可选 | 场景标题，显示在标题栏 |

句子序号是字幕在文件里的先后位置（跳过没有时间轴或没有文字的块）。正常的 SRT
里它和 SRT 自带的编号相同。完整示例见 `fixtures/sample.words.json`。

## 数据

学习状态保存在数据目录下的 `language-learning/state.json`：

- macOS：`~/Library/Application Support/language-learning/state.json`
- Linux：`~/.local/share/language-learning/state.json`
- 设置环境变量 `LL_HOME` 时改用 `$LL_HOME/language-learning/state.json`

里面记录每集的播放位置、字幕平移量、每句的听懂/没听清标记，以及收藏的卡片
（含来源音频路径和句子序号，复习时据此回放原句）和复习调度。文件损坏时程序会报错
退出而不是覆盖它。

复习的“今天”按本地时区的日期计算（取不到本地时区时退回 UTC）。日期在 `state.json` 里存为自
1970-01-01 起的天数。

## 开发

```sh
cargo build --no-default-features
cargo test --no-default-features
cargo clippy --no-default-features --all-targets -- -D warnings
cargo fmt --check
```

音频放在默认开启的 cargo feature `audio` 后面。`src/player.rs` 定义了 `Player`
trait（播放区间 / 暂停 / 继续 / 跳转 / 当前位置）；`--no-default-features` 时使用
只走时钟、不出声的 `NullPlayer`，测试也用它。rodio 实现在 `src/audio.rs`。
