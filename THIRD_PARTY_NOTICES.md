# 外部素材と依存ライブラリ

monitor2自身のコードはMITです。外部素材の条件は以下を参照してください。

- `assets/app.ico`: 旧版 `Monitor/Embed/icon.ico` から変更せず引き継ぎ。ユーザーから出典を覚えていないとの回答があり、著作権者・利用許諾・再配布条件は未確認です。旧版で使用していたことは再利用の許諾根拠になりません。
- `assets/brands/{intel,amd,nvidia}.svg`: [Simple Icons](https://github.com/simple-icons/simple-icons)の各ブランドアイコン。2026-10-01取得。コレクションはCC0 1.0ですが、個々の企業ロゴまでCC0とは限らない旨が公式に明記されています。現在のロゴの利用・再配布が自由に許可されているとは確認できていません。CC0全文は同ディレクトリのLICENSE-CC0.txt。
- ブランド名・ロゴの商標権は各社に帰属します。CC0は商標使用の許諾ではありません。個別の条件と用途上の注意はSimple IconsのDISCLAIMER.mdおよび各社のガイドラインを確認してください。
- 実行時Rust依存: windows-sys、windows-link（Microsoft、MIT OR Apache-2.0）。MIT全文はassets/licenses/windows-rs-MIT.txt。
- ビルド専用: embed-resource（MIT）、resvgとusvg（MIT OR Apache-2.0）など。Cargo.lockにバージョンを記録しています。SVGレンダラーはアプリの実行コードには含まれません。

SVG出典: [Intel](https://github.com/simple-icons/simple-icons/blob/develop/icons/intel.svg)、[AMD](https://github.com/simple-icons/simple-icons/blob/develop/icons/amd.svg)、[NVIDIA](https://github.com/simple-icons/simple-icons/blob/develop/icons/nvidia.svg)。

## 画像の確認状況

2026-10-01時点で、使用中の4画像すべてについて配布可能との確認は取れていません。MITは企業ロゴや出典不明の旧版アイコンの権利を上書きしません。調査結果と公式の条件、対応候補は[画像のライセンス確認](docs/assets-licensing.md)にまとめています。

IntelおよびIntelロゴはIntel Corporationまたはその子会社の商標です。AMDおよびAMD ArrowロゴはAdvanced Micro Devices, Inc.の商標です。NVIDIAおよびNVIDIAロゴはNVIDIA Corporationの商標または登録商標です。monitor2が各社から承認・提携を受けているという意味ではありません。帰属表記だけで利用許諾を取得したことにはなりません。
