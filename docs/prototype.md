# 初版試作

2026-10-01時点のWindows native試作です。
README.mdの整備は後で行います。

## 実装した機能

- CPU名、全体・論理CPU別使用率。PDHの英語カウンターを使用。
- すべてのプロセッサグループを列挙し、16論理CPUを超えても画像や固定配列に依存しない構造。
- RAM総量・使用量・利用可能量。GlobalMemoryStatusExを使用。
- NVIDIA GPU名・使用率・温度・VRAM。System32のNVML DLLを動的に読み込み、列挙したGPUを表示。
- 約1秒間隔のバックグラウンド取得。UIは最新のスナップショットを描画。
- Win32/GDIによる枠なし常駐ウィンドウ。通常起動ではタスクバーに表示しない。
- ドラッグ移動、右クリックメニュー、最前面の切り替え、トレイメニューからの終了。
- DPIに応じた描画サイズ、論理CPU数・GPU数による高さ変更、画面を超える場合のホイールスクロール。
- メモリ単位はGiB。取得不可はN/Aで表示し、起動直後のCPU値もウォームアップ扱い。
- 3秒以上情報が更新されない場合はSTALEを表示。部分取得失敗はPARTIALを表示。
- GDIフォントと描画バッファは再利用し、PDH/NVML/GDIリソースは終了時に解放。

## 起動と検証

PowerShellでリポジトリ直下から実行します。

```powershell
cargo run
cargo run -- --probe
cargo run -- --preview
cargo run -- --smoke-test
cargo build --release
.\target\release\monitor2.exe
```

- 引数なし: 通常のガジェット。
- --probe: 1秒間隔で3回、取得値とエラーを標準出力へ出して終了。
- --preview: 同じ描画をタスクバーに表示。画面確認ツール向けのモード。
- --smoke-test: 常駐画面を起動し、約6秒後に正常終了。

画面内の×、Esc、または右クリック／トレイのExitで終了します。
左ドラッグで移動できます。位置はまだ保存しません。

```powershell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

## 確認した環境と動作

- Windows x64、Rust 1.98.1、MSVC、Windows SDK 10.0.26100。
- CPU: AMD Ryzen 7 5800X3D、16論理CPU。
- GPU: NVIDIA GeForce RTX 4070 Ti、ドライバー616.56。
- CPU全体・16論理CPU、RAM、GPU使用率・温度・VRAMを実機で取得。
- 確認モードで画面表示、ドラッグ移動、最前面切り替えのチェック状態、メニューからの終了を確認。
- リリース版の自動終了モード、rustfmt、Clippy、単体テストを確認。
- 性能比較は[performance.md](performance.md)を参照。

## 残っている検証と制約

- 長時間動作によるリークやメモリ推移は未検証。
- 複数GPU、複数プロセッサグループ、17以上の論理CPU、異なるDPI間の移動は実機未検証。
- NVIDIA GPUやDLLがない環境、ドライバーのリセット／GPU切断後の回復は実機未検証。
- GPU初期化に失敗した場合はN/Aを表示。再初期化は次回起動時。
- NVMLはSystem32だけを検索。別の場所にしかDLLがない環境は現時点でN/A。
- NVMLのVRAM値はWDDM／ドライバーの報告値。タスクマネージャーの値と同一とは限らない。
- ウィンドウ位置・サイズの設定保存、自動起動、独自アイコンは未実装。
- CPUの定義は% Processor Time。Windowsの他の使用率指標と完全一致するとは限らない。

## 参照した公式API

- [PdhAddEnglishCounterW](https://learn.microsoft.com/en-us/windows/win32/api/pdh/nf-pdh-pdhaddenglishcounterw)
- [GlobalMemoryStatusEx](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-globalmemorystatusex)
- [NVML Device Queries](https://docs.nvidia.com/deploy/nvml-api/latest/api/group__nvmlDeviceQueries.html)
- [LoadLibraryExW](https://learn.microsoft.com/en-us/windows/win32/api/libloaderapi/nf-libloaderapi-loadlibraryexw)

旧版のGPUソースはコピーせず、公開API宣言に合わせて最小のFFIを実装しています。
