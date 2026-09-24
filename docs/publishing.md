# Publish this checkout to the existing GitHub repository

Publishing is a manual step. Run these commands on the Pi from the finished checkout. The existing `origin` is `https://github.com/PixelCoda/AOG.git`; GitHub currently redirects it to `ktheindifferent/AOG`. Do not create a different repository or force-push.

## Authenticate

Use a Git credential helper or SSH key with write access. If using GitHub CLI:

```sh
sudo apt-get install gh
gh auth login --hostname github.com --git-protocol https --web --scopes workflow
gh auth setup-git
```

Authenticate as an account with write access to the existing AOG repository. Keep tokens out of commands, repository files, and chat. Revoke the token previously shared in chat.

The additional `workflow` scope allows publication of the included GitHub Actions workflow. See [GitHub CLI authentication](https://cli.github.com/manual/gh_auth_login) and [GitHub OAuth scopes](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/scopes-for-oauth-apps).

## Publish the committed v2 tree

```sh
cd /home/aog/AOG
git status --short
git fetch origin
git push --atomic origin main main:master
```

Both branch updates are ordinary fast-forwards, preserving all prior commits. Updating the old default `master` branch as well as `main` makes v2 the current version on either branch; v1 stays in history. If Git reports a non-fast-forward or branch protection restriction, stop and review the remote changes or use a pull request. Do not use `--force`.

In GitHub **Settings → General → Default branch**, select `main`. Optionally remove the redundant `master` branch through GitHub after changing the default and reviewing branch protection. That removal is not required to publish v2.

## Publish firmware downloads

After the code is pushed and checks pass:

```sh
gh release create v2.0.0 firmware/package/*.hex firmware/package/manifest.json firmware/package/sources.zip \
  --repo ktheindifferent/AOG --target main \
  --title 'AOG 2.0.0' --notes-file docs/release-2.0.0.md
```

This creates the release tag from the remote `main` and attaches six HEX profiles, checksums, and firmware sources. Alternatively, use **Releases → Draft a new release** in GitHub, select `main`, use tag `v2.0.0`, and upload the same files. Check that the tag does not already belong to a different release.

[GitHub CLI release documentation](https://cli.github.com/manual/gh_release_create).
