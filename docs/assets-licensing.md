# 画像のライセンス確認

2026-10-01確認。結論は「現状の全画像が自由に利用・再配布できるとは確認できない」です。出典が確認できること、コレクションにライセンスがあること、個々の画像の利用許諾があることを分けて判断します。

## 使用中の素材

| 素材 | 確認結果 |
|---|---|
| assets/app.ico | 旧版Monitor/Embed/icon.icoから引き継ぎ。ユーザーは出典を覚えていない。著作権者・許諾・再配布条件が不明 |
| assets/brands/intel.svg | Simple Icons由来。Intelは第三者のロゴ利用にライセンスまたは書面による許可が必要と案内 |
| assets/brands/amd.svg | Simple Icons由来。AMDのブランド素材許諾はAMDプロセッサを含む製品の広告・販売等に限定。一般的な監視ソフトへの適用は未確認 |
| assets/brands/nvidia.svg | Simple Icons由来。NVIDIAの一般の商標案内は許可を前提とし、ロゴの公式描画指針もある。現在の用途・形状が許諾されているとは確認できない |

他の旧版の背景・バーPNGは使用していません。背景・グラフ・バーはmonitor2のGDIコードで描画します。Segoe UIはWindowsのインストール済みフォントを参照し、フォントファイルはアプリに同梱していません。画像のICO変換や色変更によって、元の画像の権利が消えることはありません。

## Simple IconsのCC0の範囲

[公式Disclaimer](https://github.com/simple-icons/simple-icons/blob/develop/DISCLAIMER.md)は、コレクションがCC0でもすべてのアイコンがCC0とは限らず、個別のライセンスとブランドガイドラインを確認するよう説明しています。個別ライセンス情報が見当たらないことも、制約がない根拠にはなりません。

以前の資料にはコレクションのCC0と商標の注意を記載していましたが、それだけで現在のロゴが自由に使えると判断するには不十分です。CC0／Disclaimerの同梱や帰属表記だけでは各社の利用許諾を補えません。

## 各社の公式情報と現在の用途

- [Intelのロゴ利用案内](https://www.intel.com/content/www/us/en/support/articles/000015080/programs.html): 第三者によるロゴ利用にライセンスまたは書面の許可が必要と記載。monitor2向けの許諾は確認できていません。
- [AMD Brand Assets Terms & Conditions](https://www.amd.com/en/legal/terms-and-conditions/media-library.html): AMDプロセッサを含む製品の広告・マーケティング・販売等に限定した許諾で、承認・スポンサー関係を誤認させないことなども条件。これをmonitor2へ適用できるとは断定していません。
- [NVIDIA Legal Notices](https://www.nvidia.com/en-us/about-nvidia/legal-info.html): 商標の公の使用は許可を前提とし、fair useについても帰属表記を含む案内があります。ハードウェア識別という用途だけで自動的に条件を満たすとは判断していません。
- [NVIDIAのロゴ指針](https://www.nvidia.com/en-us/about-nvidia/legal-info/logo-brand-usage/): ロゴの形状変更や目のマークと文字の分離を避ける指針があります。現在はSimple Iconsの目のマーク単体を表示しており、公式ロゴ指針との適合も未確認です。

この調査は、現状の用途が違法であると断定するものではありません。確認した条件と、不明なままの許諾・用途の範囲を記録しています。実際の配布地域・用途に応じた例外の適用まで検証していません。

## 対応候補

1. 出典不明のアプリアイコンを新しい自作アイコンに交換し、制作物とライセンスを記録する。
2. 企業ロゴを、通常フォントによるメーカー名と独自のCPU／GPUアイコンに交換する。ロゴの模写・意匠の再現はしない。
3. 企業ロゴを継続する場合は、対象用途の利用・再配布・色／形状の条件を確認し、必要な許諾を取得する。

公開・配布を予定する場合は1と2を推奨します。今回の確認では画像やアプリの挙動は変更していません。外部コード・依存ライブラリのライセンス記録は[THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md)を参照してください。
