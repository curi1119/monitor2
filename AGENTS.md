# AIエージェント向け作業指針

monitor2はRust製のWindows専用CPU・RAM・NVIDIA GPUモニターです。軽量な常駐動作と、小さくても読める画面を目指します。

## 作業の入口

作業前にgit statusを確認し、既存の変更とユーザー設定を保持してください。[開発手順](docs/development.md)を読み、変更する機能に応じて以下の資料を参照してください。

| 資料 | 内容・読む場面 |
|---|---|
| [開発手順](docs/development.md) | 環境構築、実行、検証、性能測定、編集・コミットの方針 |
| [構成と設定](docs/architecture.md) | モジュールの役割、スレッド、設定保存と反映、終了処理 |
| [監視データ取得](docs/monitoring.md) | CPU・RAM・GPUのAPI、単位、物理コア集計、取得失敗 |
| [描画](docs/rendering.md) | レイアウト、フォント、DPI、アイコン、Win32/GDIの寿命 |
| [使用方法と設定](docs/settings.md) | ユーザー向けの操作と設定項目 |
| [CIとリリース](docs/ci.md) | GitHub Actions、バージョン、配布ZIP |
| [外部素材・ライセンス](THIRD_PARTY_NOTICES.md) | アイコンの出典、依存ライブラリ、商標に関する条件 |
| [画像のライセンス確認](docs/assets-licensing.md) | 使用中の画像の許諾・出典の未解決点、公式条件と対応候補 |
| [性能比較](docs/performance.md) | 測定条件と結果、原データへのリンク |
| [改修履歴と検証](docs/phases.md) | 実装の経緯、実機確認済み・未検証の項目 |
| [全体レビューの修正](docs/review-fixes.md) | f1027ecのレビュー指摘6件と修正・検証結果 |

[初版試作](docs/prototype.md)は過去の記録です。そこでの候補・推奨案を現在の確定仕様と混同しないでください。

## 共通方針

- Windows native、x86_64-pc-windows-msvcを基本にします。不要な依存・取得処理・毎回の割り当てを増やさず、性能は実測で判断します。
- 監視とUIを分離し、取得失敗を正常な0値と区別します。unsafe・FFIには所有権、寿命、スレッド、再入の前提を記載します。
- OS・GPU APIは公式資料で確認し、実機未検証の制約を明示します。外部コード・素材の出典とライセンスを記録します。
- .editorconfigと.gitattributesに従い、Rustはrustfmtで整形します。Cargo.lockはGit管理します。
- 変更に応じた検証を行い、結果と制約を報告します。仕様を変えたら対応するdocs/も更新します。
- 説明・資料は日本語を基本にします。README.mdは利用者向けに簡潔に保ち、詳細はdocs/へ分離します。
- CPU温度はユーザーの指示で延期中です。再開時はRust単体での取得を優先して調査します。
- ユーザーが許可するまでコミットしません。許可されたコミットは対象の変更だけを含め、目的が分かる短いメッセージを付けます。

AI向け補助資料は.agents/、再利用するスキルは.agents/skills/<skill-name>/SKILL.mdに置きます。個人用メモはGit管理外の.agents/local/を使い、秘密情報や個人環境の設定をコミットしません。
