# Linux multi-device enrollment validation

Manual/scripted validation for issue #103 until GTK UI automation is added.

## First device setup

1. Start a fresh server with migrations applied.
2. Launch Linux with an empty key store/profile.
3. Open Sync setup, enter server URL/email/password, and choose **Login / Register**.
4. Expected: account registers, local device key and account data key are created, UI reports sync-ready, and normal sync can push/pull encrypted blobs.

## Add a second Linux device

1. Launch Linux with a separate profile/key store.
2. Open Sync setup with the same account credentials and choose **Login / Register**.
3. Expected: UI says **Signed in — waiting for approval from an enrolled device** and explains that private keys/plaintext account data keys never leave devices.
4. On the first device, open Settings → Sync → Device enrollment and choose **Refresh pending requests**.
5. Expected: pending row shows device name, platform, request time, and short public-key fingerprint.
6. Choose **Approve** only after verifying the request. Expected: first device wraps the account data key locally for the requested public key and uploads only wrapped payload material.
7. On the second device, choose **Check approval / complete**.
8. Expected: second device downloads the approved payload, verifies it is addressed to its public key, unwraps and stores the account data key locally, clears pending enrollment, and becomes sync-ready.

## Reject/recovery path

1. Create another second-device request.
2. On an enrolled Linux device choose **Reject**.
3. Expected: the request is removed from the pending list; the new device remains waiting and can retry login/request creation.

## Security expectations

- Server stores device public keys, metadata, and wrapped account-key payloads only.
- Private keys and plaintext account data keys never leave Linux devices.
- Approval requests include the recipient public key; the server rejects approvals whose recipient does not match the pending request.
- Repeated pending creation for the same user and public key returns the existing pending request instead of creating duplicates.
