# monitor2 開発引き継ぎ

> この文書は刷新前の分析・計画を残した引き継ぎです。2026-10-01以降の試作状況は[docs/prototype.md](docs/prototype.md)を参照してください。

## 目的・ユーザーの希望

- 個人用のWindows CPU／メモリ／NVIDIA GPUモニターガジェットを刷新する。
- 既存リポジトリとは別に、`monitor2`などの名前で新規プロジェクトを作る。
- 軽量・低メモリを優先する。Rustでの実装を検討している。
- GPUコードはオープンソースの類似プロジェクトからコピーしたもの。

現時点では既存コードの静的分析と開発環境の検討まで完了。新プロジェクトの作成・実装、Windows実機での動作確認・性能計測は未実施。Rustや描画方式は有力候補であり、最終決定ではない。

## 参照するリポジトリ

- 既存コード：`D:\home\monitor`（WSLでは `/mnt/d/home/monitor`）
- リモート：<https://github.com/curi1119/monitor>
- 分析対象コミット：`e40cf03`（2023-01-28）
- 新プロジェクト：`D:\home\monitor2`。2026-10-01にGit初期化と最小Rustプロジェクトの作成を実施。

## 現行アプリ

C#／.NET Framework 4.8／WinFormsの単一アプリ。1秒ごとに情報取得し、146×287の枠なしウィンドウへ描画する。常に最前面、タスクバー非表示、ドラッグ移動可能。トレイメニューから終了する。位置などの設定保存はない。

| 機能 | 実装・主なファイル |
|---|---|
| CPU全体・論理CPU別使用率 | `PerformanceCounter`、`Monitor/Hardware/CPU.cs` |
| CPU名 | レジストリから取得、同上 |
| RAM総量・使用量・利用可能量 | `GlobalMemoryStatusEx`、`Monitor/Hardware/Memory.cs` |
| GPU名・使用率・温度・VRAM | NVAPI、`Monitor/Hardware/Nvidia.cs` |
| 画面・更新・移動 | `Monitor/GUI/MainForm.cs`、`MainForm.Designer.cs` |
| 画像の読み込み・バー描画 | `Monitor/GUI/Util.cs`、`ProgressBar.cs`、`CoreProgressBar.cs` |

## 作り直す際に解消したい課題

- **画像の再生成**：描画のたびにCPUロゴのBitmapを作る。旧Bitmapと読み込み用Streamの明示的な破棄がなく、不要な割り当てと解放の遅延が起こり得る。
- **論理CPU数の上限**：CPU番号からバー画像名を生成するが、画像は`bar00`〜`bar15`のみ。17論理CPU以上で存在しない画像を読み、起動に失敗する構造。レイアウトも最大16個を想定。「コア」は実際には論理CPU。
- **UIの停止リスク**：UIスレッド上でCPU・RAM・GPUを順番に取得するため、取得が遅いと操作も止まる。
- **取得状態の不足**：GPUがない場合やAPI失敗時を、0%や前回値と区別できない。GPUは列挙順の先頭だけを使用。
- **古いGPU API**：VRAM取得にディスプレイハンドルを使用。現在の公式API宣言は物理GPUハンドルで、`NvAPI_GPU_GetMemoryInfo`は非推奨。移植時に現行仕様と照合する。
- **不要な取得・割り当て**：画面に出さないGPUクロックも毎秒取得し、取得用配列を毎回生成する。
- **表示の固定**：DPI自動調整を無効化し、座標・サイズ・フォントを固定。メモリの表示単位`Mb`は計算上は`MiB`で、RAMの「Free」は利用可能量を表す。

## 推奨する開発方針

Windows専用のRustアプリを第一候補とする。Win32の小さなウィンドウを自前描画し、OS APIとの接続にはMicrosoftの`windows`／`windows-sys`を検討する。描画方式はGDIを初期候補とし、見た目と実測結果に応じて判断する。

- CPU：PDHで全体・論理CPU別の使用率を取得。`PdhAddEnglishCounterW`を候補にする。
- RAM：`GlobalMemoryStatusEx`を継続使用。
- GPU：現在の公式NVAPIを基準に、必要な読み取り機能だけ実装。NVMLも候補だが、対象GeForce／RTXとドライバーで対応・値の意味を実測する。
- 取得処理とUIを分離し、最新結果を通知して再描画する。まずは現行と同じ1秒間隔。
- 取得不可を`N/A`などで扱い、正常な0%と区別する。
- 論理CPU数に応じたレイアウト、DPI対応、ウィンドウ位置などの設定保存を検討する。

OpenHardwareMonitor由来の3ファイルにはMPL-2.0の表記がある。リポジトリ全体のLICENSEや`Hardware/Nvidia.cs`の出典表記はないため、コードを再利用する場合はコピー元・変更箇所・ライセンスを整理する。

## 次に着手する作業

1. Windows側のRustビルド環境と対象GPUを確認し、別リポジトリに最小プロジェクトを用意する。
2. CPU・RAM・GPUの取得を試作し、対応APIと値の意味を実機確認する。
3. 最小の常駐画面を追加し、同じ更新間隔の現行版とCPU時間・Working Set・Private Bytesを比較する。
4. 長時間動作でリソース使用量が増え続けないか確認し、その結果から性能目標と描画方式を決める。

開発はWindows nativeで動くCodexを推奨。アプリの統合ターミナル設定と、エージェントの実行環境設定は別なので注意。WSLは必要なツールや分析作業の補助として利用できる。

## 参考資料

- [Microsoft Rust for Windows](https://github.com/microsoft/windows-rs)
- [NVAPI使用率取得](https://docs.nvidia.com/nvapi/group__gpupstate.html)
- [NVAPIメモリ取得の公式ヘッダー](https://docs.nvidia.com/nvapi/nvapi__lite__common_8h_source.html)
- [Windows版Codexの実行環境](https://learn.chatgpt.com/docs/windows/windows-app)

