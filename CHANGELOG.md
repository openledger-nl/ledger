# Changelog

## 1.0.0 (2026-11-22)

### Breaking changes

- **meetings:** a meeting note's headings no longer carry a time or #project tags; a note written before reads those as part of the title. Set a meeting's time and projects again in Ledger. ([d0af5c9](https://github.com/openledger-nl/ledger/commit/d0af5c9e724eeb7200526412a653eaa0ef3be5c5))
- **deliverables:** `descriptions:` in project.md is no longer read. A deliverable's description is in ([8a36d24](https://github.com/openledger-nl/ledger/commit/8a36d24a85a4725c271fad4bb0f287cb1a63b975))
- **deliverables:** a vault's versions/ and history/ folders from before are not read. Published versions must ([0803b17](https://github.com/openledger-nl/ledger/commit/0803b178702f09de03f844723db56aa07f3d2402))
- **templates:** Ledger no longer adds its own project plan template to every vault. A new vault starts with ([c56a576](https://github.com/openledger-nl/ledger/commit/c56a576c4e743b07fcd15474fb676a6dff90c3fa))
- **clients:** a work project's `client:` in project.md is a client's id in shared/clients; a name that is ([cf77f00](https://github.com/openledger-nl/ledger/commit/cf77f00c0a097a4f17b77334d95512443747423d))
- **projects:** kind: company and the role client are no longer read; a ([0ee789d](https://github.com/openledger-nl/ledger/commit/0ee789db0f1de26cf5a4f3287bf555d4a3dbaa85))
- **people:** people are read from shared/people/ and attached by id; a ([b266968](https://github.com/openledger-nl/ledger/commit/b2669680b9dd55187d51dd0da1c53984770c106f))
- **vault:** templates, PDF layouts and semesters are read from their ([203a47b](https://github.com/openledger-nl/ledger/commit/203a47bf01bce27d3c908a191594457537bf31c6))
- **settings:** settings made on this machine for a vault are not carried ([87811da](https://github.com/openledger-nl/ledger/commit/87811da08dfd16b3fc2c410dc50f6e463f058d56))
- **sync:** GitHub sync is gone. Keep the vault in a folder your cloud ([deb54c2](https://github.com/openledger-nl/ledger/commit/deb54c2ae3d9ed284b5dd940546c900019d695f0))
- **vault:** Ledger 1.0 doesn't open vaults made by Ledger 0.x and has no ([12101c4](https://github.com/openledger-nl/ledger/commit/12101c4578f8780bf091d4c1384279838a70f007))
- **meetings:** meeting notes now live in meetings/<year>/<month>/. Update Ledger on every machine ([709b06a](https://github.com/openledger-nl/ledger/commit/709b06aa6ace8dc9e616970869be4ed2f900399a))
- **meetings:** meeting notes now live in meetings/. Update Ledger on every machine that shares the ([eb6cca9](https://github.com/openledger-nl/ledger/commit/eb6cca93542ef6ed06bb3d14f46fb9c0dfc5aca9))
- **meetings:** meeting notes now live in Meetings/. Update Ledger on every machine that shares the vault: an older ([cf01b32](https://github.com/openledger-nl/ledger/commit/cf01b32bc9191d9612187af810605e210b01b98c))
- **deliverables:** published versions move from each project's deliverables/ folder to its versions/ folder, which ([68b75e4](https://github.com/openledger-nl/ledger/commit/68b75e403c340d99c7dd0386ccb71f11d19dddc5))
- **crypto:** a branch encrypted by an earlier version is no longer synced; on every machine, change the branch under Sync to GitHub to one that doesn't exist yet, and Ledger uploads the vault there with its names hidden. ([da2dac5](https://github.com/openledger-nl/ledger/commit/da2dac5106dbb973242b46a1138e45594337d40d))
- **prompts:** prompt files in the vault now keep the base text (`base`) and the feedback to resolve (`feedback`), ([5460aeb](https://github.com/openledger-nl/ledger/commit/5460aeb6a3aeec48c47328785b72e2ce0fd0ed52))
- **settings:** a machine that still keeps its settings (such as the vault folder) only in the nl.molenbrugge.ledger ([5330026](https://github.com/openledger-nl/ledger/commit/5330026fd13ee47a5315129799efaaf281e9f6d6))

### Features

- **meetings:** link who was at a meeting to their file in shared/people ([82f29d4](https://github.com/openledger-nl/ledger/commit/82f29d48d825f2e1843576cdb0ab421c088c21d8))
- **links:** link to another project's things and to people ([0449219](https://github.com/openledger-nl/ledger/commit/0449219b19a24c0db3c6c9e7290b4d89a1ea5ab0))
- **links:** link meeting notes and documents both ways ([908cb98](https://github.com/openledger-nl/ledger/commit/908cb98149ba638314ab61e9a835bb56957ebcf8))
- **ui:** a project in the explorer always goes to its page ([ba3a710](https://github.com/openledger-nl/ledger/commit/ba3a7107a4c9470231c09087d26276c0cf06d885))
- **settings:** give the keyboard shortcuts a tab of their own ([34b168f](https://github.com/openledger-nl/ledger/commit/34b168f04cee190a868222e79fd951da89f54ce9))
- **projects:** edit a project from one Edit button, with Delete under a menu ([6e7ef77](https://github.com/openledger-nl/ledger/commit/6e7ef77495ddecefe5d93474f6f50bf39a5b05d5))
- **onboarding:** ask what Ledger is for first, then set up a first project of that kind ([734ee92](https://github.com/openledger-nl/ledger/commit/734ee92c4ac69291f0388f99a1a376840495d13e))
- **planning:** keep semester templates in shared/templates/semesters, NJ26 the first ([611a952](https://github.com/openledger-nl/ledger/commit/611a952be1cb2f9180398a7301755d30ff07f0fd))
- **templates:** move the shared templates out of Settings, under Shared ([c346703](https://github.com/openledger-nl/ledger/commit/c346703a691022378bb51ed9a6b5693a8c01223a))
- **crypto:** encrypt a vault at rest ([83fcfe4](https://github.com/openledger-nl/ledger/commit/83fcfe4a1a116770dbeb411ec3b5ff013d00c44f))
- **vault:** keep a vault safe in a folder that cloud storage syncs ([30ae64d](https://github.com/openledger-nl/ledger/commit/30ae64d459ea02fa62397556d62a2fcc5af0b2d0))
- **ui:** show Semesters and People under Shared in the explorer ([4d57369](https://github.com/openledger-nl/ledger/commit/4d573696191044b1c323762b7f8a134e76e55e54))
- **planning:** give semesters a page of their own ([241a1d6](https://github.com/openledger-nl/ledger/commit/241a1d67cea77e747e1457098f88ef81ba79deaf))
- **onboarding:** drop the GitHub sync step from the setup guide ([0570c9e](https://github.com/openledger-nl/ledger/commit/0570c9ea50f08c44d5e41ec17bcb6af77e2fad66))
- **onboarding:** ask what Ledger is for and only ask school for a semester ([ada6cc7](https://github.com/openledger-nl/ledger/commit/ada6cc7404d840f398743699fff476fe0d848778))
- **ui:** drop the Latest section from the explorer ([e92a120](https://github.com/openledger-nl/ledger/commit/e92a1202dbcf7200126da44c0d4f9d133cb3be51))
- **portflow:** remember the Fontys login and sign in by itself ([e1f544a](https://github.com/openledger-nl/ledger/commit/e1f544a66e8955ac31bfd436b60873d632835235))
- **deliverables:** show the changes in the text while you write ([1f46dd7](https://github.com/openledger-nl/ledger/commit/1f46dd781a9542036cfb570caefebb621ffe8ea7))
- **claude:** choose the model for a run, and the usual one from a list ([ea630ed](https://github.com/openledger-nl/ledger/commit/ea630ed269997b87abcc742617d575139acc2c36))
- **deliverables:** show what changed between versions and who wrote it ([5dc0479](https://github.com/openledger-nl/ledger/commit/5dc04799c643215739fb0ac726550797c4e71678))
- **ui:** open a command palette with Ctrl+P and give any command a key ([3a50b5a](https://github.com/openledger-nl/ledger/commit/3a50b5a26671f8005c8ceea161c24e464b424b68))
- **deliverables:** let a project or the vault set its PDFs in a Typst layout of its own ([62cc4f5](https://github.com/openledger-nl/ledger/commit/62cc4f58498c095b9cbb1d71f289e5f0fe6c4dec))
- **deliverables:** show the AI-use label on a PDF's cover ([9f5e6a2](https://github.com/openledger-nl/ledger/commit/9f5e6a21bfee09cf89d7903231906882693eaa9e))
- **projects:** give a project's PDFs your name, both logos and an accent colour ([5520cb8](https://github.com/openledger-nl/ledger/commit/5520cb84eadead79ab3675d8a921c2d8f157cd85))
- **deliverables:** export a markdown deliverable as a PDF, set with Typst inside Ledger ([3983849](https://github.com/openledger-nl/ledger/commit/3983849125169f1ae5267a56b7bb5ae1b4cc8fca))
- **vault:** choose the vault at the first start, and switch vaults ([df15771](https://github.com/openledger-nl/ledger/commit/df15771568365ddfc3d5d9531598ee2d5696647c))
- **threads:** show a deliverable's template, feedback and prompt in the thread's side window ([5014a14](https://github.com/openledger-nl/ledger/commit/5014a14b1dd1f0d3e846914b5171be40dba13e4d))
- **search:** search the open thread from the top bar's search box ([136d5f2](https://github.com/openledger-nl/ledger/commit/136d5f26e14319cdfcb710725cd20a207ea4e4fb))
- **deliverables:** write any deliverable's evidence description where you work on it ([83e4ccc](https://github.com/openledger-nl/ledger/commit/83e4cccc7086bd37e59868b65123246e843df07f))
- **threads:** keep a thread's deliverables in the same side window as a deliverable's ([38b705c](https://github.com/openledger-nl/ledger/commit/38b705c933452628b31406c30ccbfc3a3354ec72))
- **templates:** answer the guiding questions where the deliverable is started, not in the thread ([ae759fe](https://github.com/openledger-nl/ledger/commit/ae759feffafb02a60a384f5a00571b38cb12d0de))
- **templates:** ship a basic project plan template ([3d29b3d](https://github.com/openledger-nl/ledger/commit/3d29b3d4470b2c1e5282eab241c5d7f5e601d299))
- **templates:** say who a template's deliverable is for, and which kinds of project it serves ([5e010c7](https://github.com/openledger-nl/ledger/commit/5e010c7ebd206a1f5166e6550471c336a8503661))
- **ui:** show notes between %% marks only in source mode ([ecc1cc5](https://github.com/openledger-nl/ledger/commit/ecc1cc5b7d72ac329feb308febe3b190fd2c8054))
- **threads:** open a deliverable from its thread, and find the way back ([3bb4077](https://github.com/openledger-nl/ledger/commit/3bb4077befef63b440d22be09ae61d40892260e1))
- **deliverables:** a calmer deliverable page, renamed from its menu and reached through its thread ([bfe290e](https://github.com/openledger-nl/ledger/commit/bfe290e0ee7c30181b4d525f36572067b4084531))
- **feedback:** say who feedback on a deliverable is from, and pick its thread there ([824f960](https://github.com/openledger-nl/ledger/commit/824f960150046d151bbf9ecd5a09b774ca0a3ccf))
- **deliverables:** rename a deliverable, keeping its versions and feedback ([5127caf](https://github.com/openledger-nl/ledger/commit/5127cafdce42510d4ba9544fb1eab0aaea340db0))
- **threads:** open the live deliverable, or the version, from a version-published note ([ce61ece](https://github.com/openledger-nl/ledger/commit/ce61ecebe9d68b3249de2f2cb505e4dfaaddd441))
- **threads:** show a thread's deliverables in a side window beside its timeline ([9438cab](https://github.com/openledger-nl/ledger/commit/9438cabb0b089393b915b0b5b44fbca634940daa))
- **meetings:** mention someone in a meeting's notes by typing @ ([1b4bd06](https://github.com/openledger-nl/ledger/commit/1b4bd06c9bb70bd772ad561fa1d63fafc36a222c))
- **uploads:** preview the pictures of a drop before it is saved ([cfa0841](https://github.com/openledger-nl/ledger/commit/cfa08413b6e595352297e9cfb328f37ca61f9a64))
- **uploads:** show JPEG XL pictures ([e922687](https://github.com/openledger-nl/ledger/commit/e9226877baae9f0d95f761d236ab93eeb8406d8d))
- **portflow:** edit an evidence item's description as What? / Goal? / Why it contributes? ([0c837f6](https://github.com/openledger-nl/ledger/commit/0c837f6629590461d26297e105336ca15a8e098f))
- **portflow:** keep each evidence item's description in step with Portflow ([a871e9d](https://github.com/openledger-nl/ledger/commit/a871e9defcd319d1ded9f275ba6966f02139d471))
- **portflow:** make every evidence item a deliverable, named after it ([82605dd](https://github.com/openledger-nl/ledger/commit/82605ddd7499ace8a5cc66e014d52c26b197461c))
- **portflow:** follow a reading guide's @ mentions and add them by typing @ ([1257979](https://github.com/openledger-nl/ledger/commit/1257979d15a39df77990c5c456593c7b4c207b97))
- **portflow:** keep the @ mentions of a reading guide, both ways ([be6febd](https://github.com/openledger-nl/ledger/commit/be6febdf95a7a6f62e8a009d6e6dc3232418ce3b))
- **logs:** start and stop a debug capture with Ctrl+Shift+D ([f0cbe44](https://github.com/openledger-nl/ledger/commit/f0cbe440b6c2432283392d4983b2933bbea934e4))
- **portflow:** say on the Overview that the reading guide is the collection's description ([ea35692](https://github.com/openledger-nl/ledger/commit/ea356927380c69dd9c63dcb6a86ccbbeb5f3a6c6))
- **portflow:** keep a study project's reading guide and its collection's description one text ([43beae8](https://github.com/openledger-nl/ledger/commit/43beae84b18214cc279eba5c34c0320f0e2381a4))
- **portflow:** switch on writing to Portflow, one kind of write at a time ([df5beed](https://github.com/openledger-nl/ledger/commit/df5beede342ce87a0202599b55169893262780d9))
- **portflow:** put the reading guide first on a project in your portfolio ([14f21b9](https://github.com/openledger-nl/ledger/commit/14f21b9f5a568734c7f119dbe009544e876c334d))
- **portflow:** write a project's reading guide to its collection's description ([e72aab0](https://github.com/openledger-nl/ledger/commit/e72aab06b8b07cd139cb5d5f727b5717749ede04))
- **people:** give a project's people a tab of their own ([d45a3a6](https://github.com/openledger-nl/ledger/commit/d45a3a6891fb672184a3e09b3804cd6fe6975c91))
- **uploads:** drop files onto the window to keep them in a thread ([769a535](https://github.com/openledger-nl/ledger/commit/769a53512fbe523d69ae473c1dbe0d0516a224b9))
- **ui:** make the v1 screens part of Ledger, no longer a preview on mock data ([d2b159c](https://github.com/openledger-nl/ledger/commit/d2b159cd02f439928a531b0d6d50af07e775cd88))
- **meetings:** keep meeting notes as daily notes in Daily/ ([44bd280](https://github.com/openledger-nl/ledger/commit/44bd2804453ef75f30d3acdd9d689469d5424c62))
- **feedback:** see all feedback in one inbox and record it from there ([6b32b6f](https://github.com/openledger-nl/ledger/commit/6b32b6f86129bd5ace9a2608a84460ff7b63dc95))
- **deliverables:** keep the intended label and why with a version published under another ([ac82756](https://github.com/openledger-nl/ledger/commit/ac8275678a811d12f0b3baeb62ce28e31f8211a1))
- **deliverables:** number deliverables and keep their goal in the project file ([6786d29](https://github.com/openledger-nl/ledger/commit/6786d29c4b3898a1cda072f03e38ccac51ac3213))
- **threads:** keep a post's kind and labels in its file ([fb97614](https://github.com/openledger-nl/ledger/commit/fb976149c770e8f771ff97917b4ffc12d4ff9ba9))
- **people:** name a project's coach, teammates and client ([fe84d57](https://github.com/openledger-nl/ledger/commit/fe84d574ce77c3f7f1a3b2aeb8345f3fad0b3050))
- **projects:** keep a project's kind, goal and end in its project file ([a9a9830](https://github.com/openledger-nl/ledger/commit/a9a9830bbb33f3570dd76e2c6f70c7b5e54a26e7))
- **portflow:** show Portflow and its Canvas sign-in under the sync status ([c138cda](https://github.com/openledger-nl/ledger/commit/c138cda25e8cc6cf0ea512501ae2d75fc905ddeb))
- **settings:** open a settings tab at the panel a link names ([8d33a0d](https://github.com/openledger-nl/ledger/commit/8d33a0d78bd6862d464066c0019f76558429e2ce))
- **ui:** preview the v1 screens on mock data ([e146d7a](https://github.com/openledger-nl/ledger/commit/e146d7ab071f4bb79d842db61165d14b6419179f))
- **search:** show results as you type ([d57c060](https://github.com/openledger-nl/ledger/commit/d57c060a03f9d0efa36ea428c155365ab7fe3364))
- **deliverables:** show the official MMM labels and choose one by its picture ([fe36048](https://github.com/openledger-nl/ledger/commit/fe36048bfd1411a2c72c06147cbefb5b99435d42))
- **ui:** write days as 4-11-2026, in Amsterdam time ([71bcd24](https://github.com/openledger-nl/ledger/commit/71bcd244e88394552649bfc38a1facbe963ecaee))
- **ui:** pick from Ledger's own dropdown everywhere, and type to filter it ([b12c378](https://github.com/openledger-nl/ledger/commit/b12c378cd76088aec8675a7526fa18e80c90b343))
- **portflow:** rename Gamma to Epsilon+ ([81ca605](https://github.com/openledger-nl/ledger/commit/81ca6050e5c5a13e527462538c24dc2ebfc24d3d))
- **settings:** choose whether a folded sidebar becomes an icon rail or hides ([8a40e9d](https://github.com/openledger-nl/ledger/commit/8a40e9d58e8dca10c624164fddb29d7ad593cfa7))
- **ui:** fold the sidebar and open it from the top bar on narrower windows ([1671fd7](https://github.com/openledger-nl/ledger/commit/1671fd79a384831ab7023d510b4ba7771d157840))
- **portflow:** see your HBO-i competences per semester in Epsilon+ ([26aeb1f](https://github.com/openledger-nl/ledger/commit/26aeb1f9aec610a60d92537aeb4b14b344b64c6c))
- **portflow:** bring your Portflow portfolio into Ledger, read only ([ed36d13](https://github.com/openledger-nl/ledger/commit/ed36d13771a8d90cefc7e765f3ce2b525d281b18))
- **uploads:** list a project's attachments in a tab of their own ([e4cda8c](https://github.com/openledger-nl/ledger/commit/e4cda8c482fb9ebe94f9d92e56aef88bd0fcf5fa))
- **links:** link threads, deliverables and feedback with [[links]] ([e52347e](https://github.com/openledger-nl/ledger/commit/e52347ea4f247653cb158998bbdd752b965f18b5))
- **updates:** hold a development build's changes until you choose to update ([efe1adf](https://github.com/openledger-nl/ledger/commit/efe1adfa11b281b0a218b056696dcfc9672de00d))
- **onboarding:** guide a new client from an empty vault to a first deliverable ([f25f98c](https://github.com/openledger-nl/ledger/commit/f25f98c1d10716d9c943f6e3cc10061aaaffa8e7))
- **deliverables:** give the deliverable page a side window for its summary, feedback and prompts ([a72eed4](https://github.com/openledger-nl/ledger/commit/a72eed426276138fa9e7d79dbd27ce29130bb5f4))
- **feedback:** light up the passages feedback is about and offer a comment on a selection ([57bcc52](https://github.com/openledger-nl/ledger/commit/57bcc527fa7fcd6bab9b679aa38aa43c787f3b85))
- **claude:** name a new prompt from what it asks ([335348c](https://github.com/openledger-nl/ledger/commit/335348c97702c92c91b9a8dd0a56d717bb855d9c))
- **feedback:** keep the passage a comment is about and the version it was given on ([67d29ba](https://github.com/openledger-nl/ledger/commit/67d29bab78e2f26c3ee639e26af8483104890107))
- **ui:** add a menu that opens a list next to a button of your own ([3809f05](https://github.com/openledger-nl/ledger/commit/3809f0541135e152f087daecd4695173ef7bcef3))
- **ui:** move the latest threads and deliverables below the projects ([9336911](https://github.com/openledger-nl/ledger/commit/9336911cfcb8defd21e495a78dc762e0e206efea))

### Fixes

- **vault:** name a conflict copy by the time on this computer's clock ([a94616b](https://github.com/openledger-nl/ledger/commit/a94616b9fed17cc8b0b4b5a23755ba468121bfa7))
- **screenshots:** name a screenshot by the time on this computer's clock ([bd2f98f](https://github.com/openledger-nl/ledger/commit/bd2f98f510a025cf8170d14838523bea2a5ea726))
- **vault:** say a vault has lost its ledger.json instead of calling it a 0.x vault ([4a99340](https://github.com/openledger-nl/ledger/commit/4a993407d53264593e12c7cdd40b0cf8cfdb127a))
- **links:** keep [[links]] working when a thread or deliverable is renamed ([71a066a](https://github.com/openledger-nl/ledger/commit/71a066adbf027938378343288f756e8d249f1a7f))
- **projects:** keep a project's own semester, by its id ([0950f75](https://github.com/openledger-nl/ledger/commit/0950f75b1d94550432feace5b221091cb42af133))
- **vault:** take a post's, prompt's or other item's id from its file name only ([4f34940](https://github.com/openledger-nl/ledger/commit/4f34940129a0d01d8aa587ca1a1116078fe2cfe0))
- **deliverables:** show code in an exported PDF as typed ([5310d07](https://github.com/openledger-nl/ledger/commit/5310d070740979f3dc669e9ea5ea0a3bb976e043))
- **deliverables:** export a document with a horizontal rule above text as formatted, not raw ([1986995](https://github.com/openledger-nl/ledger/commit/1986995f7e0b9001d18c421718f6292193d523e5))
- **vault:** know the files of shared/clients and the semester templates as Ledger's ([a838327](https://github.com/openledger-nl/ledger/commit/a838327bbac07fe60ff87258f1a99ccb71e2ccbd))
- **templates:** read a project's stakeholders as the client reader ([cc8360f](https://github.com/openledger-nl/ledger/commit/cc8360f05ad63346bae9a76280075d65b45e9072))
- **ui:** point to Shared for semesters and templates, not Settings ([173d0d9](https://github.com/openledger-nl/ledger/commit/173d0d9c5e454f3afe254c5734ef916136ad4a33))
- **vault:** count the conflict copies apart from the Conflicts page's list ([01e123f](https://github.com/openledger-nl/ledger/commit/01e123fd8cdaecdda6588cf9918f4bfd02188fa8))
- **vault:** say in Settings that changes from elsewhere are read in at once ([91f4115](https://github.com/openledger-nl/ledger/commit/91f41150c7e653b45315817bf3993d0eb4558bdb))
- **portflow:** stop showing the previous vault's Portflow after switching vaults ([46fc9f2](https://github.com/openledger-nl/ledger/commit/46fc9f29aa2a2f5d5b1ad4261baf163a84f6f090))
- **sync:** recover from a poisoned lock instead of closing Ledger ([5e54c62](https://github.com/openledger-nl/ledger/commit/5e54c62df9f6ec232cb08e8b165fb5a6f3069d25))
- **portflow:** sign in through Fontys ICT's new Canvas address ([768d6ea](https://github.com/openledger-nl/ledger/commit/768d6eac9a71f70b140efc44bd808c5c3ad5386e))
- **updates:** keep the dev build from running out of file watchers ([76f7496](https://github.com/openledger-nl/ledger/commit/76f74965da3597517c6150023632bd0af27100b5))
- work out today, and the day of a moment, in Amsterdam time everywhere ([644d2b0](https://github.com/openledger-nl/ledger/commit/644d2b046cf026654b0f8dfb8d9c3ac134e9f243))
- **deliverables:** size a PDF table's columns by what they hold ([1284d3c](https://github.com/openledger-nl/ledger/commit/1284d3cfb17f673cbf797f18c2dc40cffbe1b39f))
- **ui:** stop the sidebar flickering as it folds and unfolds ([0d97e3f](https://github.com/openledger-nl/ledger/commit/0d97e3f4ce27254272d64d85f606d2c9e10d6f30))
- **ui:** open the drawer on a narrow window after folding the sidebar ([8057ca5](https://github.com/openledger-nl/ledger/commit/8057ca5db407dfef446969664235ac6839befb60))
- **ui:** open a side window's drawer below the page header, beside its rail ([ba77b4e](https://github.com/openledger-nl/ledger/commit/ba77b4ec214a1bdab0280033570bb7f3aaca84f4))
- **feedback:** feedback on a deliverable goes to its own thread, and waits on a Portflow item ([135296f](https://github.com/openledger-nl/ledger/commit/135296f7fa19d695616517e3a996bbebf3a9ce48))
- **deliverables:** lead back through Threads when a deliverable is opened from its thread ([2555496](https://github.com/openledger-nl/ledger/commit/25554965a45d5f3c8b5c3eb9b604acbd4dcbab01))
- **threads:** give a post one label at most: Fact, To check or Assumption ([5be3a40](https://github.com/openledger-nl/ledger/commit/5be3a407c4c523b0947a8b8e2a7570f24ec26ee6))
- **uploads:** let Tauri take dropped files on Linux and macOS, so a drop gets its files ([b7328af](https://github.com/openledger-nl/ledger/commit/b7328af0beddfb1004bae231a6302f6dcd416be3))
- **uploads:** say when a drop is a link from a web page, and log what it held ([eef62e4](https://github.com/openledger-nl/ledger/commit/eef62e4455665a45c41c6ca7a4b96634117afc15))
- **ui:** show what a sync brought in, also where an editor holds its own copy ([7286412](https://github.com/openledger-nl/ledger/commit/7286412e6c65cd074cf88d15d746496b0954bbb6))
- **portflow:** take a linked project's reading guide from Portflow on the first sync ([9ad4d7c](https://github.com/openledger-nl/ledger/commit/9ad4d7cfce4eb05d536020b0b8d006d3f88325ad))
- **uploads:** hear a file dropped outside a thread, and never fail a drop silently ([42a7d29](https://github.com/openledger-nl/ledger/commit/42a7d29d9effe9148195ff6bbf5a02b02bfe0996))
- **uploads:** catch every file dragged in, so a dropped PDF no longer replaces Ledger ([062a3d8](https://github.com/openledger-nl/ledger/commit/062a3d876f9b2ea4106465fd837fde8d7a4064e8))
- **deliverables:** show the file name a new deliverable really gets ([18da718](https://github.com/openledger-nl/ledger/commit/18da7182d75d83be03d25465007e03c940785cdb))
- **deliverables:** date a deliverable by its newest version, not its file ([0fe3470](https://github.com/openledger-nl/ledger/commit/0fe3470b00f868d4fdbd97f8bf1f1931e635d53c))

### Other changes

- **brand:** give the app the orange icon of the default Sepia theme ([b8525b5](https://github.com/openledger-nl/ledger/commit/b8525b5116529fab80408c3756dc3bc69c713f0a))
- **threads:** make a "version published" note a link to the version's manifest ([6f0c31d](https://github.com/openledger-nl/ledger/commit/6f0c31d2a842dff6147fc1174a06ee230f36061e))
- **vault:** call the AI-use label aiUse and intendedAiUse in the files ([a068968](https://github.com/openledger-nl/ledger/commit/a068968e04a9a14c8486348fa1fe5a90b374f701))
- **settings:** let the shortcut search fill its row ([72d853f](https://github.com/openledger-nl/ledger/commit/72d853fd7d0f4bd40e48eff4436a3a3a30decb27))
- record the shorter list of known duplicate code ([346f108](https://github.com/openledger-nl/ledger/commit/346f1088f824bd03706c3380af5be176866c0058))
- describe clients, semester templates and the shared Templates page ([54cb576](https://github.com/openledger-nl/ledger/commit/54cb57686e764af8fa12ffee117d5a728b393421))
- **onboarding:** share the project picker of the thread and deliverable steps ([b072d7b](https://github.com/openledger-nl/ledger/commit/b072d7bcb0bc4147d1c2cff97e87d5235a70cb78))
- **deliverables:** read a project's live deliverables in one place ([758d54f](https://github.com/openledger-nl/ledger/commit/758d54fe74aee1f7505eb21c9a8706b739e3caa7))
- **trello:** give the burst test room for slower vault writes ([b099a82](https://github.com/openledger-nl/ledger/commit/b099a82e9b3c6281cd6ebc844596feb36534ad78))
- **ui:** keep the vault picker's checkboxes their own size ([ccb4427](https://github.com/openledger-nl/ledger/commit/ccb4427860cdd2a22794a8282fdda250ad9a85e9))
- drop GitHub sync's state from Storybook's setup ([b46d503](https://github.com/openledger-nl/ledger/commit/b46d503195628218737fa04baf07c08c94bba435))
- record the shorter list of known duplicate code ([13bf8e6](https://github.com/openledger-nl/ledger/commit/13bf8e6742ae2dfc28f139c770bc0f3a2d91a01a))
- **ui:** listen to the backend's events through one helper ([e10a54d](https://github.com/openledger-nl/ledger/commit/e10a54d854f2038c79b569cad6ae0eebfea85216))
- **vault:** open files and decode pictures through one helper each ([74f5769](https://github.com/openledger-nl/ledger/commit/74f5769d11c2b97113a3184fd5c6f32c06c64015))
- describe the 1.0 vault: cloud folders, shared/, people and encryption ([1d8b21d](https://github.com/openledger-nl/ledger/commit/1d8b21d7b77173007d578d3275138e364a59c0c2))
- **vault:** drop what only vaults from Ledger 0.x needed ([169adcf](https://github.com/openledger-nl/ledger/commit/169adcfd0ed151672457b7788fa8e38aec88ec53))
- **portflow:** drop the repairs for what Ledger 0.x left ([122fa9d](https://github.com/openledger-nl/ledger/commit/122fa9d33639acbeda75163266434de6b688caa3))
- move the workflows to the actions' Node 24 versions ([1bfe5dc](https://github.com/openledger-nl/ledger/commit/1bfe5dc403a52a35e89aebf965d13a9dbdff9314))
- explain alpha pre-releases in the README ([8215cd0](https://github.com/openledger-nl/ledger/commit/8215cd09e6e516b86862170379dd80b2c61654ef))
- build alpha pre-releases from the alpha branch ([eec78f3](https://github.com/openledger-nl/ledger/commit/eec78f3d88d749f527ba699e9e8f0eaf7a555afc))
- warn about unchecked indexes and duplicate code in the checks ([d9d1910](https://github.com/openledger-nl/ledger/commit/d9d1910991d415fdfe7583dab2eeae0566213a44))
- fail clippy on unsafe code, unwrap and expect in the Rust code ([5752529](https://github.com/openledger-nl/ledger/commit/57525299b95554ac40d6d00d922f8aed6c56fe31))
- drop the explorer's Latest section from the README ([10be604](https://github.com/openledger-nl/ledger/commit/10be6048221519d2e1ed0d70e823589566ed123f))
- **deliverables:** describe the changes in the text and the view icon ([6aaf420](https://github.com/openledger-nl/ledger/commit/6aaf4203ae13267ca0f92e0816d65bced70954fd))
- describe choosing the Claude Code model in the README ([1efc3b0](https://github.com/openledger-nl/ledger/commit/1efc3b015136a9099767a4fdbef14d09c8b646ca))
- **deliverables:** describe the history folder and the changes views ([6cb7dad](https://github.com/openledger-nl/ledger/commit/6cb7dad76277bddf4c080d285564bc5ac717cb08))
- list the command palette's module in the README's layout ([29a9d32](https://github.com/openledger-nl/ledger/commit/29a9d32225b83156b736a46ea5cffa30bb004eb6))
- **deliverables:** move the PDF settings to the project's Templates tab ([5c48bdc](https://github.com/openledger-nl/ledger/commit/5c48bdc66c95ab171e51a0082177012b48ae0560))
- **deliverables:** smaller logos on a PDF's cover, and both in every footer ([9321520](https://github.com/openledger-nl/ledger/commit/9321520bae8c9536fd8981d635626d90e9ed1393))
- **vault:** describe the vault picker and the slimmer sidebar footer ([7068395](https://github.com/openledger-nl/ledger/commit/7068395e82b5b9ffd20d12b7f49725f85c2cf6a4))
- **ui:** slide the sidebar open and closed ([4aac4fd](https://github.com/openledger-nl/ledger/commit/4aac4fd4ffb065798399023c4515b88a05aeb00e))
- show the logo at the top of the README ([cfbd0b1](https://github.com/openledger-nl/ledger/commit/cfbd0b11c5e5cc2c1e2e3cb6f6f527e3c95ba1ff))
- **brand:** new logo, a laptop seen from the side, in green ([1dfdb33](https://github.com/openledger-nl/ledger/commit/1dfdb33e77fc0af3cdaf3a736b06e00a142e7414))
- check UI text for typographic punctuation ([06c3c73](https://github.com/openledger-nl/ledger/commit/06c3c7353852ef015875118349ecca19358927c9))
- use plain punctuation in what the backend says and writes ([ca3bcd8](https://github.com/openledger-nl/ledger/commit/ca3bcd867390ec82bea14f129194aaa5026871e8))
- **ui:** use plain punctuation in the app's text ([74710bf](https://github.com/openledger-nl/ledger/commit/74710bf92ab95f73f049be43605ee7b1026ce95f))
- **ui:** pick an AI-use label from a row of six, not a dropdown ([79fd35d](https://github.com/openledger-nl/ledger/commit/79fd35d57e49ea2a1a95fe7e2f71a517383c4a1d))
- **portflow:** leave the word files off an evidence item's card ([e606e0d](https://github.com/openledger-nl/ledger/commit/e606e0d1032bf4772198379d269010cd6625015e))
- **threads:** give the note box's kind and label a row of their own ([4dc0ea6](https://github.com/openledger-nl/ledger/commit/4dc0ea6aaaa6a94d4edbbe46cb2bfef3117c5094))
- **ui:** one calm header and one row of buttons on every card ([ef6905d](https://github.com/openledger-nl/ledger/commit/ef6905dea8fbeee7ea161797b35d372345133e38))
- **deliverables:** give the deliverable page's header room below its buttons ([9fdb61b](https://github.com/openledger-nl/ledger/commit/9fdb61b9862c696dac54cba53d61f3952d8fdfa2))
- **ui:** give every input a white background on the light themes ([d3f4487](https://github.com/openledger-nl/ledger/commit/d3f4487d6e598da7d31695290539b6a96014a507))
- **ui:** the main pages in Storybook, with their layouts and data ([92f39c5](https://github.com/openledger-nl/ledger/commit/92f39c5b4ebe15c5dec9331c1c91aaee0e5501f7))
- **ui:** a story for every component, in its states ([608873d](https://github.com/openledger-nl/ledger/commit/608873d463c65e7e95af4231af65295b5565295e))
- add Storybook for the UI, on a sample vault without the backend ([a35cdb5](https://github.com/openledger-nl/ledger/commit/a35cdb51f5944e6f15aa02a88219d34b8cce63d2))
- keep the target-verify build folder out of git ([d15fe88](https://github.com/openledger-nl/ledger/commit/d15fe882aa7be3e1e60e31cce079d7ebee544183))
- **prompts:** call the thread's prompt button "Make deliverable", without dots ([9b0bcfb](https://github.com/openledger-nl/ledger/commit/9b0bcfbb2df9a4acd2f16d73caf5a6925f552ea5))
- **feedback:** make a piece of feedback calmer to read ([279938d](https://github.com/openledger-nl/ledger/commit/279938dbe1fb95d44f8f2266a985ec6fbed557ca))
- **threads:** take the counts line and the post filters off the thread page ([38a6242](https://github.com/openledger-nl/ledger/commit/38a6242c861fd9683b8456989c8c1b503264ac38))
- open the front end in a browser during development, with pnpm run dev starting the app ([25f8d6b](https://github.com/openledger-nl/ledger/commit/25f8d6bdc00a6f2b811b32cd9f66da6a1e7b4b3e))
- **ui:** put Settings beside the button that hides the sidebar ([7b6b65d](https://github.com/openledger-nl/ledger/commit/7b6b65dd79076ae36b71335f2832d31d55cb4c31))
- **meetings:** show people and projects by name, not by their tags ([badebe1](https://github.com/openledger-nl/ledger/commit/badebe1a836cd85929aacb4a564dbf8fd06f2c27))
- **meetings:** take the file path and save state off a day's notes ([8e62090](https://github.com/openledger-nl/ledger/commit/8e62090690c613f4af7ae2ed49a0cc6a790b91c7))
- **feedback:** mark the feedback helpers only the tests use ([026f2c6](https://github.com/openledger-nl/ledger/commit/026f2c6d1635d1f35e947ebfe6b93bad78b191e2))
- **ui:** let pages fill the width of the window ([42a65a9](https://github.com/openledger-nl/ledger/commit/42a65a91356ede88f4ff0debcfc69dd1828ac854))
- **portflow:** plan Epsilon+ and intended development ([031f063](https://github.com/openledger-nl/ledger/commit/031f06395b9ecde3754b19d3d823f04110381178))
- **planning:** name sprints on the timeline and line up their dates ([6f3b9ab](https://github.com/openledger-nl/ledger/commit/6f3b9ab6e824be0326effdf7e4e6601c93b9c593))
- **portflow:** plan the Portflow integration ([9ee66f6](https://github.com/openledger-nl/ledger/commit/9ee66f6ef6e0781822aa8615cb01e3a6ffac16cc))
- add the portflow commit scope ([5ef265e](https://github.com/openledger-nl/ledger/commit/5ef265e885aabb358d122bd1fdd57cd9446f4465))
- **ui:** remove the semesters button from the home page ([8a0cee8](https://github.com/openledger-nl/ledger/commit/8a0cee8b539a6fd60acbba670d8c2c1873c3e38f))
- **brand:** name sepia as the default theme in the README ([5117655](https://github.com/openledger-nl/ledger/commit/5117655d34018c703dc42c2ebede243e00d264d9))
- **brand:** make sepia the default theme ([9f5143b](https://github.com/openledger-nl/ledger/commit/9f5143b69e94c5e33445115fa45764ad4c16649e))

## 0.8.0 (2026-11-03)

### Features

- **projects:** add latest threads and deliverables to overview and explorer ([21203eb](https://github.com/openledger-nl/ledger/commit/21203eb691ed370080afb211e4676aa3d1136293))
- **sync:** add a GitHub sync card to the dashboard ([fa9c222](https://github.com/openledger-nl/ledger/commit/fa9c222f95875cf609ecbe1520449d687be84712))
- **ui:** unfold one project at a time in the explorer, with a Threads tab ([2dc7762](https://github.com/openledger-nl/ledger/commit/2dc77627ee476db0beb0a491e00e05a6f15218f3))
- **dashboard:** scope the contribution graph with sprint pills ([fc5fb93](https://github.com/openledger-nl/ledger/commit/fc5fb93a66628d7f93fc02fa1f178338e78bb06d))
- **planning:** add the NJ26 semester preset ([b8bfde6](https://github.com/openledger-nl/ledger/commit/b8bfde68a941abd606d3b724d82bc8a58eb78635))
- **settings:** move semesters and templates into settings ([690c3cf](https://github.com/openledger-nl/ledger/commit/690c3cf49d6c138740a2775a1ab7de200ac634fa))
- **planning:** plan semesters with vacations and sprints ([0a50ab0](https://github.com/openledger-nl/ledger/commit/0a50ab09c9e794182701d82d2ae207d8506005a5))

### Fixes

- **dashboard:** show the days left in the graph and tidy the cards ([19f448a](https://github.com/openledger-nl/ledger/commit/19f448ae2bb67a1e9c7fd5e70b7da8db19a5a178))

### Other changes

- add the dashboard scope ([c70c1e3](https://github.com/openledger-nl/ledger/commit/c70c1e355971992393a11b5fde6efaca76fec562))
- group stories, board and semesters under one planning scope ([f643c59](https://github.com/openledger-nl/ledger/commit/f643c597b895642a16f00402efeb5712421c1ac0))

## 0.7.1 (2026-11-03)

### Other changes

- set the updater public key ([efa944e](https://github.com/openledger-nl/ledger/commit/efa944e58ea5fe5dad850c98a31760265beaf814))
- sign updates with a key that has no password ([dafb6be](https://github.com/openledger-nl/ledger/commit/dafb6be5869d60b722f0473487b2e0221a0dfd88))

## 0.7.0 (2026-11-03)

### Features

- **settings:** show what Ledger is and where it keeps things in the About tab ([b09b75a](https://github.com/openledger-nl/ledger/commit/b09b75ab2eebbea3fa93f18f6a5cc84272aa29bf))
- **logs:** log errors on this machine and report them from a new Logs tab ([b701f71](https://github.com/openledger-nl/ledger/commit/b701f711ccd15e2276bad1887d3607c1017812b4))
- **settings:** group the settings into tabs, with a search and on/off switches ([49edeb3](https://github.com/openledger-nl/ledger/commit/49edeb3681319411d2bcf2eced6445ebb67ea31a))

### Other changes

- change the app identifier to nl.openledger.ledger ([c6c1e3b](https://github.com/openledger-nl/ledger/commit/c6c1e3bffb3fa8b331d925d9849e33dea060d057))

## 0.6.0 (2026-11-03)

### Features

- **updates:** install new releases from inside the app ([485fd0e](https://github.com/openledger-nl/ledger/commit/485fd0ef02dcb863dbf0789a7908724088254ada))
- **deliverables:** switch between source, live preview and reading, as in Obsidian ([d9a19d1](https://github.com/openledger-nl/ledger/commit/d9a19d18a0b6978b9b8cef80f94f42cdcc0008e7))
- **deliverables:** write a version's changelog in the markdown editor ([c9132c8](https://github.com/openledger-nl/ledger/commit/c9132c8b1a948b96eb36cb67353799f2fbf1ede8))
- **ui:** write markdown in an inline CodeMirror editor everywhere ([8adcbf4](https://github.com/openledger-nl/ledger/commit/8adcbf4ceade87e554cec49adaf15419c02a325f))

### Fixes

- **ui:** draw tables in the live preview and show pictures from the vault ([4d03d83](https://github.com/openledger-nl/ledger/commit/4d03d83a186f15714329ad32af760647ed1df811))

### Other changes

- push the release commit with a deploy key so the main ruleset lets it through ([e81017a](https://github.com/openledger-nl/ledger/commit/e81017a132cbd737de9f4d1c7f4cf43eb4240d93))
- check that every commit in a pull request is conventional ([e2e7df8](https://github.com/openledger-nl/ledger/commit/e2e7df8ada8581e907e3c23fdffd3565378e8f22))
- add a contributing guide ([a548042](https://github.com/openledger-nl/ledger/commit/a548042ec8a72ed5b4ca818e8f96ae30de4e6382))

## 0.5.0 (2026-11-03)

### Features

- contribution graph and dashboard update ([5a72938](https://github.com/openledger-nl/ledger/commit/5a72938ba1adba6e6a90d8572bd743b7e83aabc3))

## 0.4.0 (2026-11-03)

### Features

- **deliverables:** quote the changelog in the version published note ([0a4fa99](https://github.com/openledger-nl/ledger/commit/0a4fa998de2f8df69bec8edf39db8a234618b669))
- **settings:** one global order for posts, threads, feedback and versions ([6dd7e32](https://github.com/openledger-nl/ledger/commit/6dd7e32bbd735555abd2e12524ec1bed90b93fe4))
- **claude:** add a setting to run a prompt as soon as it is created ([b697886](https://github.com/openledger-nl/ledger/commit/b697886d6719ab2a939bdee25f8594b43145d1e3))
- **deliverables:** show how a deliverable was produced in a window beside it ([e7fa06a](https://github.com/openledger-nl/ledger/commit/e7fa06a7e6805623f90d34989bc3bd6934efe905))
- **deliverables:** note the prompt behind a version and show labels in the list ([2cb550a](https://github.com/openledger-nl/ledger/commit/2cb550a4b63292bbdcc9dcdb1ac4d34bef481669))
- **deliverables:** record how each published version came to be ([123eff6](https://github.com/openledger-nl/ledger/commit/123eff6776a3b3d6a4d3202a924133815c6b37b3))

### Fixes

- **deliverables:** do not guess who made kept edits, and keep a run with its own prompt ([37ce47a](https://github.com/openledger-nl/ledger/commit/37ce47a18c32cbd06a18b4111ddef6fae332fdb4))

### Other changes

- **deliverables:** use Lucide icons and colour tokens in the produced-by window ([6e983ec](https://github.com/openledger-nl/ledger/commit/6e983ecf38fec764c7d3b9549654778138904905))

## 0.3.0 (2026-11-03)

### Features

- **brand:** let the logo take the theme's accent ([9b66b95](https://github.com/openledger-nl/ledger/commit/9b66b95d621994cfdc8ee3adcd5f040db02be32d))

### Other changes

- list the commit types and scopes in CLAUDE.md ([50a0cab](https://github.com/openledger-nl/ledger/commit/50a0cab9a33984f8adf60c40611bdffd37e58955))
- typecheck and test every push and pull request ([5cc5c40](https://github.com/openledger-nl/ledger/commit/5cc5c40f867c156bfdb22255b40ba54c46bfe31c))
- move to Node 26 ([87fb3dd](https://github.com/openledger-nl/ledger/commit/87fb3dd0b35ce2423de5006938979a035a560fec))

## 0.2.0 (2026-11-03)

### Features

- **threads:** let the chat use the full width ([5c87405](https://github.com/openledger-nl/ledger/commit/5c874055ec817dc57870a504789ba6c3aa43622a))
- **ui:** use Lucide icons instead of text glyphs ([91762d9](https://github.com/openledger-nl/ledger/commit/91762d9eddcd18644396b8ccf8213bce6aa57c7d))
- **screenshots:** bring your own screenshot tool, such as Flameshot ([2534690](https://github.com/openledger-nl/ledger/commit/2534690730be5a10e58c0341675c7151c2680e97))

### Other changes

- release every push to main from its conventional commits ([7a8f96a](https://github.com/openledger-nl/ledger/commit/7a8f96a1272a167f0515d4137ec6cd46c81422ff))
