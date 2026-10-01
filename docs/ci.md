# ビルドとリリース

`.github/workflows/ci.yml`: pushとpull_requestでWindows 2022上の整形、Clippy、テスト、releaseビルドを実行します。

`.github/workflows/release.yml`: GitHub ActionsのReleaseでRun workflowを選択し、Cargo.tomlと同じバージョン（例0.1.0）を入力します。検証後、EXE、利用方法、MITライセンス、外部素材の条件を含むZIPとSHA256を作り、v付きタグとGitHub Releaseを作成します。draftをオンにすると下書きになります。既存タグの上書きは行いません。ワークフローをデフォルトブランチに配置してから手動実行してください。

ローカルでの配布物作成:

```powershell
cargo build --locked --release
./scripts/package-release.ps1 -Version 0.1.0
```

出力はtarget/dist/。Gitリモートoriginはgit@github.com:curi1119/monitor2.gitです。

## ZIPの構成

配布ZIPはmonitor2.exe、LICENSE、readme.txtの3ファイルです。readme.txtの原稿は[release-readme.txt](release-readme.txt)で、プレーンテキスト・UTF-8・LFです。

配布用LICENSEはプロジェクトのMIT全文とwindows-sys／windows-linkのMicrosoftの著作権表示・MIT全文を統合して生成します。リポジトリ直下のLICENSEは変更しません。[MicrosoftのMIT原文](https://github.com/microsoft/windows-rs/blob/master/license-mit)は著作権・許諾の表示をコピーに含める条件があるため、別ファイルを省いても全文は残します。

THIRD_PARTY_NOTICES.md、使用方法.md、windows-rs-MIT.txtはZIPには入れません。CC0の全文コピーには同梱条件がなく、Simple IconsのDisclaimerにも配布物への同梱を義務付ける記述は確認できないため、LICENSE-CC0.txtとDISCLAIMER.mdもZIPから外します。素材の出典・帰属・確認状況はreadme.txtから参照できます。原文と詳細資料はリポジトリに維持します。

これは同梱ファイルの整理で、個々の画像の利用許諾が確認済みになったという意味ではありません。根拠と確認状況は[assets-licensing.md](assets-licensing.md)を参照してください。既存v0.1.0の添付ファイルは変更せず、次の配布物からこの構成を使用します。


## GitHub上の実行確認

2026-10-01、mainの59fd5feを初回pushし、[Windows CI実行36793394626](https://github.com/curi1119/monitor2/actions/runs/36793394626)が成功しました。Windows 2022上で整形・Clippy・テスト・releaseビルドがすべて成功し、buildジョブは1分58秒で終了しました。

Windows CIと手動Releaseワークフローがactiveで登録されていることも確認しました。

同日、ユーザーが[手動Release実行36793654292](https://github.com/curi1119/monitor2/actions/runs/36793654292)を開始し、2分18秒で成功しました。対象は59fd5feで、[v0.1.0](https://github.com/curi1119/monitor2/releases/tag/v0.1.0)を下書き・プレリリースではない状態で作成しています。

Release添付のZIPとSHA256ファイルを取得し、ハッシュ一致とEXE・使用方法・ライセンスの同梱を確認しました。ZIPは206,881 bytes、EXEは629,760 bytes。ZIP SHA256は49ee5dee06750825110dd1c80a58043a2f391a4d7e29f553d171766297940ca5です。GitHubビルドのEXE自体の起動検証は行っていません。

checkout v4とupload-artifact v4の固定リビジョンについて、Node.js 20が非推奨となりNode.js 24で強制実行される旨の警告が出ています。今回のチェックは成功していますが、アクション更新時に対応するリビジョンを確認してください。

参考: [workflow_dispatch](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#workflow_dispatch)、[gh release create](https://cli.github.com/manual/gh_release_create)。
