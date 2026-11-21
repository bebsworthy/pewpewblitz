# Outcome
The public Android release meets Google Play's privacy and account-deletion rules and GDPR: players can delete their account and its data, a privacy policy states what is collected and for how long, and the Data safety form matches what the server actually stores and logs.

# Found (2026-11-21)
## What PewPew Blitz stores and logs
| Data | Where | Retention today |
|---|---|---|
| Account key: a random 128-bit secret, hashed after BRL-4xjh | client connections file and server `profiles`, `character_choices`, `published_maps` | forever |
| Character picks | `character_choices` | forever |
| Display name | sent each login and not stored on the profile; **stored as `author_display_name` on published maps** | forever |
| Published map recipes, names and moderation reasons | `published_maps` | forever |
| Client IP addresses and ports | supervisor stderr lines (`ingress dropped … source=`, `capability-route-first-seen … source=`; `packages/brawler-routing/src/runtime/reporting.rs:44-66`) | container log retention (BRL-71r3) |
| Backups of all of the above | off-host bucket (BRL-kdza) | 30 days (proposed in BRL-kdza) |

No email, real name, location, contacts, payment or analytics SDK is collected; this needs confirming in the Android manifest and dependencies at implementation.

## Deletion and export
- None exist. `brawler-profile-admin` only does backup and map moderation (`src/bin/profile_admin.rs:12-17`).
- Foreign keys already cascade from `profiles` to `character_choices` and `published_maps`.

## Google Play rules
- An app that lets users create an account must let them request deletion **in the app and through a web link** entered in the Play Console.
- Deleting an account must delete its associated data.
- The Data safety form has mandatory deletion questions.
- Accounts here are created implicitly on first connect, which counts as account creation.
- Sources: [Play account deletion requirements](https://support.google.com/googleplay/android-developer/answer/13327111), [Android Developers blog, 2023](https://android-developers.googleblog.com/2023/04/giving-people-more-control-over-their-data.html), [designing account deletion, 2024](https://android-developers.googleblog.com/2024/03/designing-your-account-deletion-experience-google-play.html). Re-check the live policy page before submission.

# Decisions (owner-approved 2026-11-21)
1. **In-app deletion.**
   - Settings → "Delete my account on this server" → confirm. The lobby deletes the profile row (the cascade removes picks and published maps, including approved maps in rotation) and ends the session.
   - The client forgets its stored account for that server; the next connect creates a fresh account.
   - Deletion is refused while the account holds a running-match seat, with the message to finish or leave the match first.
2. **Web deletion path:** a static page served by Caddy on the BRL-3he3 domain. It explains the in-app steps and offers an email request with the in-app **account reference**, a short code the client shows under Settings and derived from the hashed key (never the secret). The operator deletes by reference with a new `brawler-profile-admin accounts delete <reference>`. An email request is honoured within 30 days.
3. **Access and export (GDPR Art. 15 and 20)** by email request with the account reference; `brawler-profile-admin accounts export <reference>` prints the account's rows as JSON. No in-app export at launch.
4. **Retention:**
   - Account data is kept until deleted.
   - Supervisor logs with IPs are kept **14 days** (BRL-71r3 sets the log driver accordingly).
   - Backups are kept 30 days, so deleted data leaves backups within 30 days.
   - Inactive accounts aren't purged at launch.
5. **Privacy policy** (a static page on the same domain, linked from the store listing and in-app Settings): controller identity and contact, data table above, purposes (gameplay, moderation, abuse prevention), legal basis (contract and legitimate interest), retention, rights, and the processor (Scaleway, EU). The owner supplies the controller name and contact.
6. **Data safety form:** declares user IDs (account key), user-generated content (maps, display name on maps), and IP addresses (diagnostics and abuse prevention); data encrypted in transit (after BRL-z277); deletion available. Record the submitted answers here.

# Scope
Decisions 1–6. A network test for in-app deletion (cascade, session end, fresh account on reconnect). Admin `accounts delete|export` with tests. Both static pages. Client Settings entries (delete, account reference, privacy policy link).

# Non-goals
Account recovery; in-app export; age gating beyond the store content rating (record the rating questionnaire answers here).

# Acceptance
- Deleting in-app removes every row for that account (test), and the next connect creates a new account.
- Admin delete and export work by reference.
- The privacy policy and deletion pages are reachable on the domain.
- The Data safety answers are recorded.

# Verification
`just lint`, `just test`, the deletion network test, and a manual Android run of delete then reconnect (recorded).
