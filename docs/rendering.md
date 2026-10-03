# 描画

## テーマ

テーマごとの配色と装飾はsrc/ui/themes/のdefault.rs／flat.rs／overlay.rsへ分離しています。デフォルトとフラットはdetailed.rsの配置を共有し、項目位置と寸法を一致させます。ui.rsは描画先・フォント・ウィンドウの管理と基本描画操作を担当します。以下の装飾・ブランド表示の記述はデフォルトの仕様です。

フラットは初期試作（7562c14）の配色を現在の配置へ適用し、バーを単色にします。グラデーション・枠・ブランドロゴは省きます。パネル間の透過と角丸の形状は維持します。

オーバレイは1行12 DIP、CPU・RAMとGPUごとのGPU・VRAM使用率の行だけを描きます。GPU1台では100×48 DIPです。背景を専用色で塗り、[SetLayeredWindowAttributes](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setlayeredwindowattributes)のLWA_COLORKEYで完全透過させます。背景色はバー・文字に使いません。文字の色に透過色が混ざるのを避けるため、オーバレイだけフォントをNONANTIALIASED_QUALITYで作り、文字はバー上に重ね、暗色の縁取りを付けます。CPU・RAM・GPU・VRAMのバー色は黄緑（#A8D848）に統一します。0%と取得失敗は0%／N/Aで区別し、GPUなしでもGPU・VRAMのN/A行を残します。

WS_EX_LAYEREDと、クリックで前面アプリのフォーカスを奪わないWS_EX_NOACTIVATEはオーバレイでのみ使い、デフォルト／フラットへ戻す際に外します。オーバレイのウィンドウ領域は表示範囲の矩形とし、ピクセルごとの背景透過はWindowsに任せます。全画面へのクリック通過は指定せず、透過した背景だけが背後へ通り、文字とバーからは操作できます。テーマ切り替え時は透明度、領域、寸法とフォントを更新します。ゲームへの注入やゲームAPIの呼び出しは行いません。

## レイアウトと文字

src/ui.rsのWin32/GDI画面です。基本幅140 DIP、16論理コアとGPU1台では高さ280 DIPです。DIPは96 DPIを基準にした座標で、描画時にウィンドウDPIへ換算します。

- CPU・RAM・コア使用率・GPU・VRAMを上から順に配置します。
- コアが8以下なら1列、9以上なら2列。コア行は10 DIP高です。
- Segoe UIを使い、見出し12、メモリ値10、機種名・補助表示9 DIPの文字高を確保します。CPU全体・GPUの使用率と温度は11 DIP・太字（weight 700）で強調します。全体使用率の表示欄も100%の幅を実測し、最低28 DIPを確保します。右端は138 DIP、CPUのY座標は18 DIP、GPUはパネル基準19 DIPです。幅が必要な倍率では左側へ広げ、バー幅を調整します。ポイント数とは異なります。
- CPU番号・コア使用率%を隠すと、その分使用率バーを広げます。%欄はフォント・DPIごとに100%の幅を実測し、最低21 DIPを確保します。バーとの間隔は1 DIPです。通常倍率では%欄が23 DIP、2列・番号なし・%ありの場合のバー幅が40 DIPです。
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

CPU・RAMと各GPUの背景を角丸パネルとして分離し、パネル間に2 DIPの完全に透過した隙間を設けます。隙間とパネルの角から背後のウィンドウが見え、そこへのクリックも背後へ通ります。CPU・RAMは1つのパネルにまとめ、各パネルから画面全体をドラッグできます。GPU1台の場合の全体の高さは従来と同じです。

CreateRoundRectRgnで各パネルの形状を作り、CombineRgnのRGN_ORで結合した後、RGN_ANDで表示領域へ切り詰めてSetWindowRgnへ渡します。寸法・DPI・スクロール位置・CPUパネルの高さ・GPUパネル数を記録し、変化したときだけリージョンを作ります。スクロールでは形状も内容と一緒に移動します。SetWindowRgnは同期メッセージを送るため、呼び出し前に形状を記録し、Appの参照を保持しません。成功時は結合リージョンの所有権をWindowsへ渡し、失敗時だけDeleteObjectします。一時的なパネル・表示領域のリージョンは結合後に解放します。

参照: [SetWindowRgn](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowrgn)、[CreateRoundRectRgn](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-createroundrectrgn)、[CombineRgn](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-combinergn)、[RoundRect](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-roundrect)。

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

起動時は保存済みのスクリーン座標でウィンドウを作成し、起動後と監視データによる高さ変更時に、最寄りモニターの作業領域内へ位置を補正します。領域より大きい場合は左上を作業領域の左上に合わせます。ドラッグ中の各移動ではファイルへ書き込まず、[WM_EXITSIZEMOVE](https://learn.microsoft.com/en-us/windows/win32/winmsg/wm-exitsizemove)で[GetWindowRect](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getwindowrect)の左上座標を保存します。復元時のモニター選択・作業領域取得には[MonitorFromRect](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-monitorfromrect)と[GetMonitorInfoW](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getmonitorinfow)を使います。複数モニター・異なるDPI間での位置復元は実機での追加確認が必要です。

現在のユーザー操作は[settings.md](settings.md)、画面変更の経緯と実機検証記録は[phases.md](phases.md)を参照してください。
