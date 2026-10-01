# 開発手順

## 環境

Windows上でx86_64-pc-windows-msvcをビルド・実行します。Linux／WSLのみでのアプリ実行は対象外です。

必要なもの:

- Git、RustupとRust stable。
- Visual Studio 2022またはBuild Toolsの「C++によるデスクトップ開発」、MSVC x64ビルドツール、Windows SDK。build.rsのリソース埋め込みにもSDKのツールを使います。
- NVIDIA GPUの実機検証には、NVMLを提供するNVIDIAドライバー。CPU・RAMの動作確認やビルド自体にはGPUは不要です。

Rustツールチェーンとrustfmt／Clippyは[rust-toolchain.toml](../rust-toolchain.toml)に指定しています。Rustup導入後、リポジトリで次を実行してください。

```powershell
rustup toolchain install stable --profile minimal --component rustfmt --component clippy
rustup show
cargo build --locked
```

cargoが見つからない場合はターミナルを開き直し、RustupのbinがPATHにあることを確認します。依存バージョンはCargo.lockに固定しますが、stableツールチェーン自体は更新されるため、性能測定や不具合報告にはrustc --versionも記録します。

## 実行と検証

リポジトリ直下で実行します。

```powershell
cargo run --locked
cargo run --locked -- --probe
cargo run --locked -- --preview
cargo run --locked -- --smoke-test
```

| オプション | 用途 |
|---|---|
| 引数なし | 通常の枠なし常駐画面。タスクバーには表示しない |
| --probe | 約1秒間隔で3回、監視値と取得エラーをコンソールへ出力 |
| --preview | タスクバーに表示する画面確認用モード |
| --smoke-test | 画面を起動し、約6秒後に終了する起動終了検査 |

これらのオプションは一度に1つ指定します。probeは設定画面の更新間隔を使用しません。release版はWindowsサブシステムのため、コンソールでの取得調査にはdebug版のprobeを使います。

コード・依存変更時の基本チェック:

```powershell
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --release
```

挙動を変えたら、変更箇所の意味のあるテスト・実機確認も行います。UIでは文字の重なり、設定切り替え、DPI、終了時の解放を確認し、未確認の環境は報告します。文書のみの変更ではリンク・記述と実装の整合・差分を確認し、ビルドの繰り返しは不要です。

実行中のrelease EXEはWindowsで上書きできません。ビルド前にそのmonitor2だけを終了してください。比較対象の旧版やユーザーが使っている別プロセスを一括停止しないでください。

## 性能測定

[scripts/measure-processes.ps1](../scripts/measure-processes.ps1)で旧版と改修版を同時測定します。PIDはその場で確認し、過去の記録の番号をそのまま使わないでください。

```powershell
./scripts/measure-processes.ps1 -LegacyProcessId <旧版PID> -PrototypeProcessId <改修版PID> -DurationSeconds 120 -IntervalSeconds 2 -WarmupSeconds 10
```

上記のPID部分は実際の数値に置き換えます。通常常駐モードのrelease版を使い、更新間隔・コアと%の設定・測定中の操作を記録します。CPU時間、Working Set、Private Bytes、ハンドル数を測定し、出力は既定でtarget/benchmarks/です。測定結果を残すときは条件と原データをdocs/に保存します。比較値と解釈は[performance.md](performance.md)を参照してください。

## 編集・資料・Git

- UTF-8、通常のテキストはLF。bat／cmdはCRLF。.editorconfig／.gitattributesを基準にします。
- Cargo.lockは追跡し、target/は生成物として除外します。依存変更を意図しないビルド・検証では--lockedを使います。
- 文書は日本語。機能仕様は該当資料へ、実測値や実機検証は条件・対象版を添えて記録します。
- [prototype.md](prototype.md)は初版コミット時点、[phases.md](phases.md)は改修の経緯です。現在の設計は[architecture.md](architecture.md)、[monitoring.md](monitoring.md)、[rendering.md](rendering.md)を更新してください。
- README.mdは利用者向けの概要・ダウンロード・基本操作を簡潔にまとめます。AI補助資料・個人用メモの配置とコミット許可のルールは[AGENTS.md](../AGENTS.md)に従います。
- 秘密情報や個人環境の設定はGitに含めません。許可されたコミットでは差分と対象ファイルを確認し、短い目的のメッセージを付けます。

CIと配布物の作成・公開は[ci.md](ci.md)にまとめています。
