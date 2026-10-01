# 描画

## レイアウトと文字

src/ui.rsのWin32/GDI画面です。基本幅140 DIP、16論理コアとGPU1台では高さ280 DIPです。DIPは96 DPIを基準にした座標で、描画時にウィンドウDPIへ換算します。

- CPU・RAM・コア使用率・GPU・VRAMを上から順に配置します。
- コアが8以下なら1列、9以上なら2列。コア行は10 DIP高です。
- Segoe UIを使い、見出し12、メモリ値10、機種名・補助表示9 DIPの文字高を確保します。CPU全体・GPUの使用率と温度は11 DIP・太字（weight 700）で強調します。使用率は旧版に合わせて右下へ2 DIPずつ移動し、CPUは(110, 18)、GPUはパネル基準(110, 19)に配置します。ポイント数とは異なります。
- CPU番号・コア使用率%を隠すと、その分使用率バーを広げます。%欄は21 DIP、バーとの間隔は1 DIPです。2列・番号なし・%ありの場合のバー幅は42 DIPです。
- CPU名の末尾のProcessor／コア数表記、GPU名の先頭のNVIDIAを表示用に省略します。元のSnapshot名は保持します。
- 文字は単行・垂直中央で描き、長い文字列は末尾を省略します。小さいフォントへの自動縮小は行いません。
- コアやGPUが増えると高さを変更し、作業領域を超える部分はホイールでスクロールします。

小型化では文字を潰さず、まず余白・列間・重複する表記を調整してください。バーだけでなく、CPU番号と%の全表示組み合わせ、長い機種名、100%やN/Aも確認します。

## 配色とバー

旧版に合わせ、背景は濃紺、文字は白／AliceBlue、CPU名は金色、GPU名はMediumSpringGreen、GPU温度はピンクです。CPU・GPUの見出し付近には16段の濃紺グラデーションを描きます。

使用率バーは上半分を明るく、下半分を暗く描き、旧版のような立体感を出します。CPU全体は青、RAM／VRAMは青緑、GPUは緑。コアは旧版の16色を表示順に繰り返します。未使用部分も明暗のあるグレーです。0%や取得失敗時はグレーのみ、100%は全幅を塗ります。

旧版のPNGを追加せず、src/ui.rsのRGB定数とstock brushの矩形描画で再現しています。フォント・配置・DPI換算は維持し、バーごとの画像やブラシの割り当ては行いません。

## 縁取りと角丸

外周は旧版に合わせた灰白色（RGB #656566）、1デバイスピクセルの線、角の半径は約3 DIPです。stock DC_PENとHOLLOW_BRUSHを選択してRoundRectで最後に描き、元のペンとブラシを復元します。CPUと各GPUを別々の枠で囲み、線を外周から1 DIP内側へ置いて角のクリッピングを避けます。枠は各パネルの内容と一緒にスクロールします。

CreateRoundRectRgn／SetWindowRgnで実際のウィンドウ形状も角丸にします。WM_SIZEと初期化時に寸法・DPIを確認し、変化したときだけリージョンを作ります。SetWindowRgnは同期メッセージを送るため、呼び出し前に寸法を記録し、Appの参照を保持しません。成功時はリージョンの所有権をWindowsへ渡し、失敗時だけDeleteObjectします。

参照: [SetWindowRgn](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowrgn)、[CreateRoundRectRgn](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-createroundrectrgn)、[RoundRect](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-roundrect)。

## GDIと描画コスト

画面はWM_PAINTで描き、互換DC・ビットマップによるダブルバッファを使います。画面サイズが変わらなければバッファを再利用し、フォントもDPI変更まで再利用します。描画のたびに監視APIを呼びません。

GDIのSelectObjectで選択した元のオブジェクトを復元してから、所有ビットマップ・DC・フォントを解放します。stock brushなど借りたオブジェクトは破棄しません。BeginPaintとEndPaintを対応させます。

## DPIとアイコン

プロセスはPer-Monitor V2のDPI対応です。WM_DPICHANGEDでフォントとブランドアイコンを作り直し、推奨位置・サイズを適用します。DPI変更処理中の再入については[architecture.md](architecture.md)の所有権の注意を参照してください。

アプリアイコンは旧版から引き継いだassets/app.icoで、ウィンドウとトレイへ埋め込みます。ブランドはassets/brands/のSVGをbuild.rsでICO化し、リソースへ埋め込みます。

| リソースID | 素材 |
|---|---|
| 1 | アプリアイコン |
| 2 | Intel |
| 3 | AMD |
| 4 | NVIDIA |

ブランドICOには28／35／42／49／56／63／70／84／112pxの画像を生成します。各サイズの4倍でSVGを描き、4×4ピクセルの面積平均で縮小します。描画前に不透明な濃紺（#101B2C）で塗り、縮小後も全ピクセルのアルファを255に保ちます。ロゴの外側には薄い枠（#3F4D63）を描きます。処理はビルド時だけで、実行時にSVG解析や縮小処理は行いません。画面では28 DIPに相当するピクセルサイズをLoadImageWで指定し、DPIに合う画像を読み込みます。用意したサイズ以外の倍率ではWindows側でサイズ調整される場合があります。

LoadIconWによる標準サイズへの縮小とDrawIconExでの再縮小を避けています。ブランド画像はサイズ別に所有し、LR_SHAREDを使いません。DPI変更時と終了時にDestroyIconします。アプリ用の共有リソースアイコンとは所有権を区別してください。

素材の条件は[THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md)を参照してください。ブランドの商標条件をコレクションのCC0と混同しないでください。

## 操作と実機確認

通常起動では枠・タスクバー表示なしの常駐画面です。左ドラッグで移動し、右クリックとトレイから設定・終了を選びます。最前面は設定に従います。設定画面はsrc/settings_ui.rsの標準コントロールで構成し、IsDialogMessageWを使います。

previewで文字・アイコン・配置を確認し、通常モードでトレイ・ドラッグ・最前面も確認します。設定画面を異なるDPIのモニターへ移動した場合、現状は開き直して新しいDPIを反映します。主画面の複数DPI環境も実機確認範囲を明示してください。

現在のユーザー操作は[settings.md](settings.md)、画面変更の経緯と実機検証記録は[phases.md](phases.md)を参照してください。
