# 外部素材と依存ライブラリ

monitor2自身のコードはMITです。外部素材の条件は以下を参照してください。

- `assets/app.ico`: 旧版 `Monitor/Embed/icon.ico` から変更せず引き継ぎ。元の制作履歴は旧版リポジトリの管理者に確認してください。
- `assets/brands/{intel,amd,nvidia}.svg`: [Simple Icons](https://github.com/simple-icons/simple-icons)の各ブランドアイコン。2026-10-01取得。コレクションはCC0 1.0。全文は同ディレクトリのLICENSE-CC0.txt。
- ブランド名・ロゴの商標権は各社に帰属します。CC0は商標使用の許諾ではありません。個別の条件と用途上の注意はSimple IconsのDISCLAIMER.mdおよび各社のガイドラインを確認してください。
- 実行時Rust依存: windows-sys、windows-link（Microsoft、MIT OR Apache-2.0）。MIT全文はassets/licenses/windows-rs-MIT.txt。
- ビルド専用: embed-resource（MIT）、resvgとusvg（MIT OR Apache-2.0）など。Cargo.lockにバージョンを記録しています。SVGレンダラーはアプリの実行コードには含まれません。

SVG出典: [Intel](https://github.com/simple-icons/simple-icons/blob/develop/icons/intel.svg)、[AMD](https://github.com/simple-icons/simple-icons/blob/develop/icons/amd.svg)、[NVIDIA](https://github.com/simple-icons/simple-icons/blob/develop/icons/nvidia.svg)。
