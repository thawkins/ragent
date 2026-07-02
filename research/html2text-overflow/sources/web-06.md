# Web source

- URL: https://changedetection.io/CHANGELOG.txt
- Title:     _                          _     _          _   _            _     
- Captured (UTC): 2026-06-29T16:21:09.798743657+00:00

```text
_                          _     _          _   _            _     
 __| |_  __ _ _ _  __ _ ___ __| |___| |_ ___ __| |_(_)___ _ _   (_)___ 
/ _| ' \/ _` | ' \/ _` / -_) _` / -_)  _/ -_) _|  _| / _ \ ' \ _| / _ \
\__|_||_\__,_|_||_\__, \___\__,_\___|\__\___\__|\__|_\___/_||_(_)_\___/
                  |___/                                                

HEAD / 2026-06-28 12:25:43 +0200:
 - LLM: strip sampling params and retry when the model rejects them (#4241)
 - fix: ensure all RSS endpoints bypass global auth when using token (#4235)
 - Restock - Threshold change since "first check" was working really as "since last check", update UI, tests, field name.
 - Restock - test fix
 - fix: extract <title> from pages with large <head> sections (#4217) (#4220)
 - Updating language catalog
 - feat: Czech translation updated and refined (#4203)
 - Update docker-compose.yml - adding LLM_FEATURES_DISABLED example

 - 0.55.7
 - UI - LLM - Fix for settings (wtforms vs pydantic) (#4184)
 - LLM - Smarter reasoning budget logic for gemini models

 - 0.55.6
 - Security - SSRF in ChangeDetection.io via urlparse/urllib3 Parser Differential
 - lint: Bump dennis — adopt `--strict` mode and drop false-positive workarounds (#4182)
 - Code - LLM settings pydantic refactor (#4181)
 - LLM UI - Blueprint/code also disabled when env flag LLM_FEATURES_DISABLED is enabled (#4180)
 - Notifications - `raw_diff` token was missing (#4177)
 - UI - LLM - Flag `LLM_FEATURES_DISABLED` to disable all LLM from the UI/system (#4171)
 - UI - Preview problem fix for extract_text/ignore_text #4138 (#4169)

 - 0.55.5
 - LLM - Master on/off switch (enable/disable) (#4162)
 - Notifications - Fix `'str' object is not callable` when `{{ diff(...) }}` callable tokens are used with HTML/htmlcolor output (#4161)
 - Fix Spanish translations for 'Changed' and 'Last Changed' (#4160)

 - 0.55.4
 - API Security - Watch GET history snapshot - Should return `text/plain` mimetype so it cant be accidently executed in the browser (#4158)
 - UI - LLM - SSRF guard for the LLM `api_base` setting (#4157)
 - LLM - UI & Ollama tweaks (#4148)
 - Update Language compilation
 - UI / LLM - Model name should not be 'read only', tidy up drop down list of providers #4115
 - Docker - INSTALLED_MARKER is kept in /datastore but package installs are not persistent (Dont use custom marker file, rely on pip instead) (#4147)
 - Fix/pr 4110 czech l12n catalog sync (#4145)
 - Text filters - Ignore text should run before 'extract text'  (#4143)
 - API - Better support for watch API private/internal vars
 - Text filters - Process subtractive_selectors first (#4142)
 - LLM - Fixing summary cache miss-hit (#4136)
 - LLM - UI - Message that 'AI Intent' (triggers) need a bigger model
 - LLM - Allow better override of formats and rules for intent/triggers
 - LLM - Remove the 'format' info from the system prompt so you can create your own 'summary' formats (ie: "Make a new JSON object with the timestamp")
 - UI - Make LLM status sticky (#4135)
 - LLM - Bumping default prompt
 - LLM integration - LiteLLM config - UI tweaks (#4134)
 - LLM - Self-hosted OpenAI-compatible endpoint support (vLLM, LM Studio, llama.cpp) — refs #3204 (#4117)
 - UI - "Time between check" fields re-order labels. #4128
 - HTML escaping in HTML notifications - Bumping tests (#4131)
 - HTML hygiene fix/improvement (Dont allow unescaped HTML to become real HTML in notifications)
 - Fixing GHSA-vwgh-2hvh-4xm5 — substring match in the shared_diff_access, improve access control to shared diff access (#4130)
 - Notifications - Escape only the diff variables before Jinja2 renders  them into the template ( Stop breaking custom HTML for plaintext pages on HTML notifications) #4121 (#4123)
 - Notifications - extra check for system default #4119 (#4122)
 - i18n: Fix broken HTML tags and enforce dennis lint warnings in CI (#4116)
 - i18n: Clear pre-existing dennis warnings in `messages.pot` (#4112)
 - i18n: Enforce dennis lint warnings in CI (#4105)
 - API - Add restock config to API /v1/watch/ json output #4099 (#4103)
 - API - watch.link was accidently a tuple, enforcing string (#4104)
 - i18n: Add dennis .pot/.po lint (#4097)
 - Improve LiteLLM deps #4093 (#4102)
 - UI - AI/LLM - "Summary" button should set last viewed (#4095)
 - Ruff INT (flake8-gettext) (#4096)

 - 0.55.3

 - Recompile languages
 - 0.55.2
 - typo: {{diff_url}} token mentioned twice (#4094)
 - i18n: UI - Align desktop "Last Checked" / "Last Changed" with mobile (#4090)
 - UI - AI/LLM - OpenRouter config UI was missing the correct fields. #4091
 - Freeze POT-Creation-Date at sentinel to stop per-locale churn (#4092)
 - i18n - Recompile languages

 - 0.55.1
 - Security - Hardening XML parser against XXE
 - Security - Stored XSS via Tag Name in Modal Dialog
 - Security - Arbitrary Local File Read via crafted backup restore
 - i18n - Update Korean language (#4084)
 - [i18n] "Usage" tab label in AI / LLM settings is ambiguous across contexts #4086 (#4088)
 - Translations - Playwright macro unused, add extra linting for translations, add TRANSLATORS.md (#4087)
 - i18n: Consolidate fragmented gettext calls into entire-sentence msgids (#4076)
 - LLM / AI Change detection rules and Summaries
 - Bumping README
 - DeprecationWarning: codecs.open() is deprecated. Use open() instead. (#4078)
 - CI - Translation sync check (#4085)
 - Update python-engineio requirement from <5,>=4.9.0 to >=4.13.1,<5 (#4079)
 - CI - Re #4080 msgfmt linting (#4081)
 - i18n: Wrap untranslated UI strings in include_subtract.html and add ja translations (#4054)
 - UI - Fix broken opacity feedback for restock/price fields on tag edit screen (#4072)
 - UI - Use pgettext for diff page From/To labels to prevent context collisions (#4073)

 - 0.54.10
 - UI - Fix unresponsive "Show advanced help and tips" button on tag edit screen (#4055)
 - Fix untranslated labels on mobile watchlist view (#4064)
 - Fix - diff_changed_to causing some missed notifications #4063 #3818 (#4066)
 - Bump apprise from 1.9.8 to 1.9.9 (#4059)
 - i18n: Wrap untranslated UI strings and update ja translations (#4052)

 - 0.54.9
 - Ignore text should override trigger text (It should ignore the trigger text if it appears) (#3450)
 - Translations - JA - Recompile
 - Fix strings not rendered in user's locale despite having .po entries (#4051)
 - Update Japanese translations for new strings and fix fragment handling (#4050)
 - Notifications - Discord #3721 - Dont use &nbsp; for discord (Actually Discord:// notifications should always use plaintext format anyway)
 - Test improvement - text extract tidyup (#4048)
 - Text filters - New simpler filter "Extract lines containing text" (#4046)
 - Handle inline favicons (#4047 #3891 )
 - UI - URL field should be just a string field (Not type=url) because URLs with Jinja2 macros could cause false errors #3777
 - Add complete Turkish translation (#4044)
 - Czech l12n updates (#4043)
 - fix: XLSX import error messages report wrong row number after failed rows + test (#4036)
 - Test - word-level diff - Re #4037 - adding test (#4042)
 - Fix/step failure notification crash (#4041)
 - Groups - Set custom colour for tag/group/label background (#4040)
 - fix: pass include_change_type_prefix to word-level diff (#4037)
 - Add Portuguese (Brasil) translation (#4033)
 - Feature - Groups/tag - Apply a group by specifying a wildcard, ie `*.mysite.com*` (#4032)
 - diff_changed_from/diff_changed_from tokens - improve documentation
 - Notification - Adding tokens `{{diff_changed_from}}` and `{{diff_changed_to}}` #3818 (#4031)
 - Fix `SCREENSHOT_MAX_HEIGHT` not enforced: cap viewport step_size and clip stitched output to max capture height #3810 (#4030)
 - UI - Minor text fix and add link to 'Restock Backup' from Imports
 - Update Selenium RemoteConnection to use ClientConfig for timeout (#4027)
 - Add Japanese translation (ja) (#4019)
 - UI - German translation: Visual Filter: "Klare Auswahl" is very misleading #4023

 - 0.54.8
 - CVE-2026-35490 - Authentication Bypass via Decorator Ordering
 - Update openapi-core requirement from ~=0.22 to ~=0.23 (#4009)
 - Ensure all unit tests are run (#4022)
 - Extendable theme pluggy implementation for main theme/template `<head>` section  (#4011)
 - Update docker-compose.yml
 - Update docker-compose.yml

 - 0.54.7
 - Translations - recompiling
 - fix: Czech translation strings updated (#4008)
 - Security: XPath json-doc() Arbitrary File Read Bypass ( Similar fix as CVE-2026-29039 )
 - CVE-2026-33981 - Environment Variable Disclosure via jq env Builtin in Include Filters
 - UI - Settings - Dont let 'password' field autocomplete (chrome)
 - `last_error` should be cleared if page content was the same and there was no error (#3997)
 - fix: correct critical errors in Spanish (es) translation (#3994)
 - Restock - Add previous_price to restock values #3987 (#3993)
 - UI - Scan/check all proxies - Regression fix from earlier refactor
 - Realtime - Suppress socket.io errors in logs (#3991)
 - UI - Text tidyup (#3989)

 - 0.54.6
 - SONP - Attempt to strip out JSONP, treat as plaintext (#3983 #3982)
 - Content Fetchers / Browsers - Improvements for pluggable extra fetchers/browsers. (#3981)
 - fix: add commit calls for pause and mute operations (#3978)
 - Bump apprise from 1.9.7 to 1.9.8 (#3979)

 - 0.54.5
 - CI - YML tidyup
 - Docker image - Improving org.opencontainers labels for dev containers
 - Docker image - Improving org.opencontainers labels #3794
 - API - Invert `changes_only` flag for include_equal parameter, add test, fixes `changesOnly` option for history diff API call (#3976)
 - UI - Fixing Preview "GO" version button (#3969)
 - API - Create (POST) tag/group through API do not save processor_config_restock_diff values #3966 (#3968)
 - Add complete Spanish translation (es) (#3961)
 - Various memory and CPU improvements (#3960)
 - CI - Bump the all group with 5 updates (#3955)
 - UI - Restock/pricing - Handle when price amount is sometimes string or integer (#3950)
 - Content fetching -Better detection of other encodings, Replace/upgrade broken UTF-8 , Ensure rest of retrieved content is UTF-8 for the app (#3954)
 - Restock - No need to extract the text because it's not used anyway (#3951)

 - 0.54.4
 - CVE-2026-29038 - Reflected XSS in RSS Tag Error Response
 - CVE-2026-29039 - XPath - Arbitrary File Read via unparsed-text()
 - CVE-2026-29065 - fix(backups): patch zip slip advisory, zip bomb, upload size limit, UUID validation, secret.txt leakage, and download edge cases
 - Updating API docs with better processor plugin info (#3942)
 - Python 3.14 CI test and support (#3941)
 - fix(i18n): accept translated confirmation text when clearing snapshot history (#3940)

 - 0.54.3
 - CVE-2026-27696 Small fix - Restricted hostnames can still be added but are only checked at fetch-time (not when rendering lists etc) (#3938)
 - Adding Ukranian translations, rebuilding translations. (#3936)
 - Update messages.po in French translation (#3926)

 - 0.54.2
 - Unresolvable hostnames should still be added, they are checked for security at fetch time (#3933)
 - CI workflow - Bump the all group with 2 updates (#3931)
 - Update jsonpath-ng requirement from ~=1.7.0 to ~=1.8.0 (#3929)
 - API - Processors configuration is now part of the API  (#3902)
 - Notification Token {{diff}} can accept arguments like `{{diff_added(lines=5, context=2)}}` (#3923)
 - Fixing `change_datetime` notification token (and adding test) (#3922)

 - 0.54.1
 - Tests - Tweaks to upgrade path tests
 - Tests - Run upgrade path test with ALLOW_IANA_RESTRICTED_ADDRESSES=true
 - CVE-2026-27696 - Server-Side Request Forgery (SSRF) via Watch URLs, set env var `ALLOW_IANA_RESTRICTED_ADDRESSES` to `true` to access IANA reserved URLs such as http://169.254.169.254, http://10.0.0.1/, http://127.0.0.1/, etc.
 - CVE-2026-27645 - Reflected XSS in RSS Single Watch request

 - 0.53.7
 - Libraries/Build - unpin referencing library (#3919)
 - Bump referencing from 0.35.1 to 0.37.0 (#3677)
 - Upgrading flask-socketio and related packages with security updates ( #3910 ) (#3918)

 - 0.53.6
 - Pip installs - remove flask patch and pin library versions
 - Lazy load flask_compress
 - UI - Content compression was not obeying FLASK_ENABLE_COMPRESSION, should be off by default due to a memory leak in flask_compress & socket.io

 - 0.53.5
 - Fixing bad replacement of metadata causing possible content removal #3906 (#3908)
 - UI - Backup restore (#3899)

 - 0.53.4
 - Updates/migration - Re-run tag update, re-save to cleanup changedetection.json, code refactor (#3898)
 - UI - Search modal - fixes for running in sub path
 - Puppeteer - Adding extra browser cleanup (#3897)
 - Puppeteer - Use a modern scroll method for screenshot stitching
 - UI - CSS - Ensure 'difference' 'preview' both wraps by word and by very long strings
 - Fix: Some SPAs with long content - Stripping tags must also find matching close tag (#3895)
 - Fix: Some SPA's also set body content to display: none which breaks text output
 - "Error 200 no content" - Some very large SPA pages make HTML to Text fail by dumping 10Mb+ into page header, strip extras. (#3892)
 - UI - Filters & Triggers - Adding reminder that you can also use 'Conditions' for trigger rules
 - Minor code tidy
 - Fix time schedule off-by-one bug at exact end times for all durations and add comprehensive edge case tests Re #846 (#3890)
 - UI - More fixes for realtime updates
 - UI - Fixing realtime updates for status updates when checking (#3889)
 - Pluggy plugin hook for before and after a watch is processed (#3888)

 - 0.53.3
 - API - Adding automated test for API with NGINX sub-path, Skip validation errors about server path (allows use on sub-paths/reverse proxy etc) (#3886)
 - UI - Use version from code in version tab

 - 0.53.2
 - UI - Watch overview - Restock price, validate number before output (#3883)
 - Security - Adding small test and fixing overzealous filename cleaner (#3884)
 - Datastore - On fresh installs, also scan for existing watch.json watches in subdirectories
 - Security CVE-2026-25527 - Unauthenticated static path traversal in resources
 - Browser Steps - Minor code cleanup
 - UI - Browser Steps - First step was missing Clear / Remove / Pic buttons

 - 0.53.1
 - Browser Steps - Clean off empty fields on save/update (UI and API), small refactor Re #3874, #3879 (#3880)
 - Test - Improve test for watch package download
 - UI - Watch data download, fix test, update text.
 - UI - Ability to download a complete data package (.zip) of a watch (#3877)
 - Disable content compression of HTML/etc by default due to memory leak between flask_socketio and flask and flask_compress.
 - Avoid reprocessing if the page was the same (#3867)
 - Update python-socketio requirement from ~=5.16.0 to ~=5.16.1 (#3869)
 - API - Remove `flask_expects_json` validation, this is covered entirely by OpenAPI, update OpenAPI spec. (#3871)
 - Update python-engineio requirement from ~=4.13.0 to ~=4.13.1 (#3868)
 - Price tracker - Use a more memory efficient price scraper, use subprocess on linux for cleaner memory management. (#3864)
 - Refactoring upgrade path (#3861)
 - API - Import use background task to import large lists (#3858)
 - API - Bumping docs
 - API - Import - Ability to set any watch value as HTTP URL Query value, for example ?processor=restock_diff&time_between_check={'hours':24}  Re #3845 (#3857)
 - API - Include missing `tags` in fetching watch information. #3854 (#3856)
 - UI - Bulk checkbox operations modal confirmation fix Re #3853
 - Tags update fix (#3849)
 - Refactor for Tags storage (#3848)
 - Including uptime in UI settings/info
 - Refactor  watch saving backend, closes #3846 (#3847)
 - Bump psutil from 7.2.1 to 7.2.2 (#3844)
 - Bump pyppeteer-ng from 2.0.0rc12 to 2.0.0rc13 (#3843)
 - Fix for When MoreThanOnePriceFound() is raised, plugins dont fire #3840 #3833
 - Rebuild translations (#3842)
 - UI - Favicon use lazy load for faster rendering
 - Adding more tests and Watch object improvements (#3841)
 - Improved watch global settings handling (#3839)
 - New datastore message should be warning not critical
 - Improving upgrade path
 - History length limit size option (#3834)
 - Memory improvement - Use builtin markupsafe instead of creating a jinja2 template env each time for small strings (#3836)
 - Favicon path - cache results
 - UI - Backups tab - styling fix
 - Styling fix for "backups" tab Re #3821
 - UI- Fix possible bug adding tags in quickwatch form
 - Processor plugin improvements - Now supports creating your own processor (for example, monitor DNS changes) (#3739)
 - Bump elementpath from 5.1.0 to 5.1.1 (#3799)
 - Puppeteer and Playwright browser close/shutdown improvements (#3830)
 - Refactor of queue systenm and improve tests, improves multiple workers (#3826)
 - Ability to limit total number of watches with env var PAGE_WATCH_LIMIT (#3828)
 - UI - Move Default Proxy selection back to "General" tab
 - API - Notification URLs werent always being validated (#3812)
 - Remove deprecated call to strtobool
 - UI - Make watch tags link elements (#3813)
 - test tweak
 - DB data migration upgrade fixes (#3811)
 - Big refactor to save watches as their own datafile with some agnostic data store backend, saves writing a huge JSON file every time (#3775)
 - Improved catching of errors/exceptions in Browser Steps steps  (#3808)
 - Improving default settings for remote reverse proxies (#3806)
 - CLI extra options,  "batch mode" see `--help` allows re-checking and adding watches from the CLI (#3802)
 - Update messages.po // German (#3797)
 - Bump apprise from 1.9.6 to 1.9.7 (#3800)

 - 0.52.9
 - Memory management improvements  for large screenshots, Brotli snapshot improvements (#3798)
 - Updating site.webmanifest for PWA usage
 - Use credentials to fetch web manifest (#3790)
 - Make language selection sticky and provide a way to return back to default auto-detect  #3792 (#3795)
 - Element locking 'off' by default (so they dont move when the screenshot scroll happens), only lock top viewport elements. Improve logging. (#3796)
 - Rebuilding language translation files
 - Update French translation (#3788)
 - Open github link on new tab (#3791)
 - Update messages.po // German "From" (#3793)
 - Improving container version labeling, using `master` branch as docker `:dev` tag. Re #3794

 - 0.52.8
 - Memory - Favicon reader had a memory leak,  Restart fetch workers between jobs, misc tweaks   (#3787)
 - API -  Validation improvements (#3782)
 - i18n - zh traditional chinese autodetect from browser fix
 - UI - Fixes for search dialog #3778 (#3781)

 - 0.52.7
 - Fix zh PO duplicates and complete new translations. (#3773)
 - Lots of translation updates (#3772)
 - UI - Global "mute" and "pause" buttons on main menu, move "Backups" to "Settings" (#3769)
 - API & UI - Recheck all - Dont requeue existing queued or processing watches. (#3770)
 - Non blocking improvements (#3767)
 - Improvements to deterministic fix (false triggers) (#3766)
 - Run "clear all history" in background thread to prevent blocking (#3765)
 - Test - Adding missing test
 - Important fix for possible wrong detection of changes under high-concurrency setups (many many fetch workers)
 - Language updates (#3764)
 - Queues and Scheduler - No need to add imported items to the check queue, the scheduler will do this #3762 (#3763), CPU usage improvements.
 - UI - Fixing link to scheduler help/tutorial page.
 - Manual update of DE language (and recompile all languages)
 - Recompile CSS
 - UI - Mobile - Empty page watches message and layout improvements (#3760)
 - UI - CSS - Give dark-mode switching a soft transition
 - Edit - More reliable fetch of watch on test (usually affects tests)
 - Manual polish for several translations in the zh locale. (#3757)
 - Fix for old selenium 3 (#3748 #3756), however be sure to use selenium 4.
 - Languages - Recompile languages, small fix for 'de'.
 - Bump elementpath from 5.0.4 to 5.1.0 (#3754)
 - Update zh translations with improved, consistent Simplified Chinese UI copy. (#3752)
 - Bump apprise from 1.9.5 to 1.9.6 (#3753)
 - 0.52.6
 - Selenium fetcher - Small fix for #3748 RGB error on transparent screenshots or similar (#3749)
 - UI - Show queue size above watch table in realtime

 - 0.52.5
 - Revert sub-process brotli saving because it could fork-bomb/use up too many system resources (#3747)
 - i18n: Recompile zh_Hant_TW/LC_MESSAGES/messages.mo
 - i18n: Update zh_Hant_TW translations (#3745)
 - Update jsonschema requirement from ~=4.25 to ~=4.26 (#3743)
 - Translations - ZH_Hant_TW - Fixing `timeago` string handling #3737
 - Translations - Fixing `zh_TW` to `zh_Hant_TW` , adding tests #3737 (#3744)
 - Bump pyppeteer-ng from 2.0.0rc10 to 2.0.0rc11 (#3742)

 - 0.52.4
 - Fixing Traditional Chinese locale mapping #3737 (#3738)
 - Languages - Pypi/pip package was missing translations

 - 0.52.3
 - UI - Groups - Adding 'Recheck' button from groups overview page
 - Minor playwright memory cleanup improvements (#3736)
 - Browser Steps UI async_loop bug, refactored startup of BrowserSteps, increased test coverage. Re #3734 (#3735)

 - 0.52.2
 - Page fetchers - Were not truely running independently and could have been blocking eachother, this commit speeds up page fetches where there is more than 1 worker.
 - RSS - Bugfix - possible edge case of wrong feed info could be rendered (#3733)
 - UI - Language modal - flag icons should be round

 - 0.52.1
 - Development branch merge into release/master
 - Adding test for #3720
 - Testing - fix: Replace time.sleep with wait_for_notification_endpoint_output in test_notification (#3716)
 - Update README.md - Info about setting up different viewport sizes
 - Use soft delays instead of blocking time sleeps in scheduler (#3710)
 - API - Watch get, retry watch data if watch dict changed (more reliable)
 - Notification debug log - Use locale of system for dates/times
 - Misc small HTML Validation fixes (#3704)

 - 0.51.4
 - Improving UTF-8 handling for xPath selectors (Stop the xpath filter from chewing up non-regulat-latin-text style content) (#3659)
 - Bump actions/checkout from 5 to 6 in the all group (#3651)
 - Specify UTF-8 encoding for xpath_element_js (#3650)
 - Update playwright library to 1.56

 - 0.51.3
 - RSS Reader Mode parser improvements - Pick up all fields from RSS where possible, better auto-detect of the XML encoding if it wasnt set by the browser (#3646)

 - 0.51.2
 - RSS - New Settings option for making RSS follow the format of `Notification Body` across watch/group/etc, or system default and override the format with your own as you like.

 - 0.51.1 Fixing semver version number

 - 0.51.01

 - 0.51.00
 - UI - Minor text fix for anon history access
 - RSS per watch tweaks (#3635)
 - RSS Feed per watch - Setting order (newest changes first) (#3634)
 - UI - Moving 'RSS' options to its own settings tab, RSS - Adding watch history length  (#3633)
 - RSS per group! (#3632)
 - UI - Move 'Jitter seconds' settings tab from "General" to "Fetching" global Settings.
 - README typo fix and ignore files for emacs style backups
 - RSS feeds for a single watches!
 - Always backup JSON DB on new versions as well as the existing between updates.

 - 0.50.43
 - Forcing UTF-8 when reading JSON DB (Fixes data not loaded for some platforms  #3622 #3611 #3628), Always create new versions of the backup DB if one exists for that step when running updates, Adding extra sanity checks on DB load
 - Adding data sanity checks across restarts (#3629)

 - 0.50.42
 - Revert "Windows - JSON DB fixes - Forcing utf-8 for json DB read/writes should solve windows saving/loading problems. (#3615 #3611)"

 - 0.50.41
 - Windows - JSON DB fixes - Forcing utf-8 for json DB read/writes should solve windows saving/loading problems. (#3615 #3611)
 - Update orjson requirement from ~=3.10 to ~=3.11 (#3617)

 - 0.50.40
 - Page <title> should only be captured on HTML documents (#3608)
 - Notification body/title - Fixing validation on empty strings #3606 (#3607)
 - Real time UI - Remove polling thread for updates - it's all done realtime by signals (#3603)
 - Watch history - Don't rescan whole history.txt when looking up a timestamp <->filepath (#3602)
 - Datastore - Use `orjson` for faster saves (#3601)
 - Scheduler - Saving a couple of CPU cycles in logging strategy

 - 0.50.39
 - Time scheduler - Remove cache on time lookup
 - Tests - Adding extra placemarker tests (#3592 #3591 )
 - Update jsonpath-ng requirement from ~=1.5.3 to ~=1.7.0 (#3586)
 - Bump actions/download-artifact from 5 to 6 in the all group (#3585)
 - Update pytest-flask requirement from ~=1.2 to ~=1.3 (#3587)
 - Update python-socketio requirement from ~=5.14.2 to ~=5.14.3 (#3588)
 - API - Adding better explanation and usage of History API, bumping doc versions.
 - API - Rebuilding HTML docs
 - API - Support optional processor on Watch create to set the restock_diff or text_json_diff mode on watch create.
 - Notifications - Adding `{{diff_full_clean}}`, `{{diff_removed_clean}}`, `{{diff_added_clean}}`, `{{diff_clean}}` notification body tokens for using in templates without (added)/(removed) text. (#3580)

 - 0.50.38
 - Improved send test notification handling (#3579)

 - 0.50.37
 - Fixing title markup in notifications (title/subject for email, slack etc), refactoring line-feed logic `\n` -> `<br>` etc (#3577) #3538 #3576
 - Dockerfile cache tweaks and build layer github cache re-enable (#3575)

 - 0.50.35
 - Notifications - Text and Markdown type was not migrated correctly to the new settings, resulting in possible non-notification, #3572 #3559 #3558 #3573
 - API - Updating index.html of the documentation
 - Optimisations to GitHub test flow

 - 0.50.34
 - Fixes to notification '`Send test notification`' (#3571)
 - HTML Notification - Adjusting font to rem size
 - Run all pytests in parallel (#3569)
 - Unify safe URL checking to the one function, strengthen tests and filters (#3564)
 - Build/test - Parallel test jobs for faster testing (#3568)
 - Handle `format=` in apprise URLs (#3567)
 - Adding small amount of cache to common functions (#3565)
 - CVE-2025-62780 - Stored XSS in Watch update via API

 - 0.50.33
 - Fixing wrong notification type in <select> that lead to wrong type of notifications (plaintext vs html) being sent #3558 (#3559)
 - HTML - Shorten whitespace around timezone names
 - Update 21 for #3496 - Fixing update of timezone setting
 - OpenAPI specification, fixing enum for notification type, and notification_muted (#3557) Re #3556
 - Update brotli requirement from ~=1.0 to ~=1.1 (#3553)
 - Update wtforms requirement from ~=3.0 to ~=3.2 (#3551)
 - Build - Actions / Bump the all group with 2 updates (#3550)
 - Update python-socketio requirement from ~=5.13.0 to ~=5.14.2 (#3552)
 - RSS - Update feedgen requirement from ~=0.9 to ~=1.0 (#3554)

 - 0.50.32
 - Tests - API - Import - Removed 'content-type': 'text/plain' from the test because this should be assumed.
 - API - Import - Automatically assume text/plain content type on Import (makes it easier for changedetection to add new URLs) #3547 #3542
 - Notifications - Keep monospaced layout of history/difference sent to HTML style notifications, Fixes to Markdown #3540 (#3544)
 - Notifications - Preserve original document whitespace in HTML style notifications (#3546)
 - Notifications - `post://', `put://` etc - Catch and show errors and where possible (#3543)
 - HTML Notification Color fixes - Reverting colors and using older style (#3545)

 - 0.50.31
 - Changes to colors HTML notification (small contrast between 'changed' and 'removed' etc) (#3540)
 - tgram:// and discord:// - Small fix for line breaks
 - Notifications fixes, extensive testing of all tokens, fixing text markup in HTML emails etc #3529 (#3539)

 - 0.50.30
 - Notifications fixes (#3534) #3531 #3530 #3529
 - Template - Adding `|regex_replace` Re #3501 (#3536)
 - Be sure that default namespaces are registered (#3535)

 - 0.50.29
 - Discord + Telegram - Adding better styling (Discord now uses strike-through and bold for removal/additions instead of broken HTML) (#3528)
 - Notifications - Refactor/cleanup notification handling and rename 'Markdown' to "Markdown to HTML" to make more sense. (#3527) Re #3526 -
 - UI - Fix watch table striping on delete #3523
 - Update flask requirement from ~=2.3 to ~=3.1, unpin werkzeug (#3502)
 - Bump elementpath from 4.1.5 to 5.0.4 (#3470)
 - Update beautifulsoup4 requirement (#3471)
 - Update validators requirement from ~=0.21 to ~=0.35 (#3500)

 - 0.50.28
 - Email notification format fixes (#3525)
 - Empty "ignore text" lines could break ignore text and prevent changes from being detected (#3524)

 - 0.50.27
 - Fix error handling for first empty filter response (#3516)

 - 0.50.26
 - pip build - Improving fix for #3509, Adding automated test for #3509

 - 0.50.25
 - pip build - Be sure to include API spec (#3511)
 - Improved watch delete (#3510)
 - Notification service improved failure alerts for filter missing + browsersteps  problems (#3507)
 - Notifications - Small fix for notification format handling, enabling HTML Color for `{{diff_removed}}` and `{{diff_added}}` (#3508)

 - 0.50.24
 - Notification - Make sure all notification tokens have something set even for form validation, fixes `hassio://` with `{{ watch_uuid }}` in notification URL form (#3504)

 - 0.50.23
 - Replace jinja2-time with `arrow` and improve timedate timezone integration, fixes timezones in templates such as `{% now 'Europe/London', '%Y-%m-%d' %}` etc (#3496)

 - 0.50.22
 - Testing - Adding test for requests timeout setting #975
 - UI - Add missing 'requests timeout in seconds' field to main settings, Re #975
 - UI - Proxy and external browser settings URL validation (#3494)
 - Move proxy default selection to proxy tab
 - Build - Splitting memory report (#3493)
 - Replace stream/filetype detection library with `puremagic`, 20Mb less RAM usage (#3491)

 - 0.50.21
 - Adding 'RSS reader mode' (see main Settings) (#3488)
 - Re #3486 - Fixing and adding test for RSS/Atom not being converted to text when server sends "text/xml" instead of the "application/atom+xml" header (#3487)
 - Ensure JSON is always correctly reformatted with padding (#3485 #3482)
 - No need to reformat/reprocess content in the case that no filters were found  (#3484,  #3483)

 - 0.50.20
 - PDF - Will trigger a change - Fixing output, also reported original size of document was incorrect (it was the size of the HTML output after conversion from PDF), Improving tests (#3481)

 - 0.50.19
 - Test speedup - remove common calls for function calls (#3477)
 - Reducing memory usage (#3476)
 - Refactoring text/html difference processor (#3475)

 - 0.50.18
 - Always follow plaintext header over the actual content type if its available (#3473) #3472
 - Bump github/codeql-action from 3 to 4 in the all group (#3468)
 - (Realtime updates) Update python-engineio requirement from ~=4.12.0 to ~=4.12.3 (#3467)
 - Bump psutil from 7.0.0 to 7.1.0 (#3469)

 - 0.50.17
 - Refactor content type detection, fixing more xpath issues for RSS types (#3465)  #3462  #3391
 - Dependabot tweaks

 - 0.50.16
 - Fixing bad detection of text text/plain in previous release, adding automated test (#3460)

 - 0.50.15
 - Build - Fixing the multi platform container build test (repairs to cache) (#3455)
 - Filters - Adding "Strip ignored lines" in output option to filters (#3449)
 - Bump apprise from 1.9.4 to 1.9.5 (#3448)
 - Build - `linux/arm64` and `linux/arm64/v8` are the same, remove v8
 - Build - Pinning library versions to fix tests
 - Notifications - Upgrade Apprise 1.9.4 (#3443)
 - Process `text/*` non-HTML in their original format keeping line breaks, auto-detect attachments/downloads for text or HTML, WARNING - Will trigger false changes for some existing text file watches #3434 (#3435)
 - UI - Implementation of unread counter - adding test
 - UI - Re #3393 #3419 Implementation of unread counter tab along with realtime updates (#3433)

 - 0.50.14
 - Time interval field - Extra validation improvements and tests (#3432)
 - UI - Fixing HTML <title> versus custom title settings display in overview (#3430) #3429
 - API - Adding page title link, bumping docs (#3431)
 - "Time between check" field is now validated correctly (requires atleast one of the weeks days hours minutes seconds to be set)

 - 0.50.13
 - API - OpenAPI call validation was being skipped on docker based installs, misc API fixes (#3424)
 - Always extract page <title>, `{{watch_title}}` added to notification body tokens (#3415)
 - UI - Correctly set 'checking now' status badge on edit page
 - Add noindex meta (#3416)
 - Build - Bump actions/setup-python from 5 to 6 in the all group (#3408)
 - Restock - Add 'nicht mehr lieferbar' to stock status checks (#3410)

 - 0.50.12
 - Fix - Filters in tags/groups were being added to watches on each check - #3406 fix list update (#3407)
 - UI - Added "unread" view filter (#3393)
 - Enable "last_viewed" field in the watch API. (#3403)
 - Update docker-compose.yml - Include mac port info warning

 - 0.50.11
 - Bump cryptography from 43.0.1 to 44.0.1 (#3399)
 - Cryptography library - pinning version
 - UI - Improving "real-time updates offline" message
 - Build - Adding new cryptography library, solving apprise plugin issues (#3398) #3397
 - Update api-spec.yaml
 - API - API endpoint call validation against OpenAPI specification YML also (#3386)
 - API Docs - Improve descriptions
 - API Doc rebuild
 - Bump API Docs slightly
 - Update settings.html text
 - API - Use OpenAPI docs (#3384)
 - Refactor API Documentation (#3383)
 - Updating API documentation
 - Favicons in list - Prefer best/highest quality (#3351)

 - 0.50.10
 - API - Recheck by tag #3356 (#3378)
 - Cleanup empty queue messages Re #3376 (#3377)

 - 0.50.9
 - Bump actions/checkout from 4 to 5 in the all group (#3373)
 - Refactoring queue handling (#3363)
 - Build - rPi - Cryptography lib not needed (#3365)
 - Build - Bump actions/download-artifact from 4 to 5 in the all group (#3364)
 - Conditions & API - Fix set Conditions by API  (#3349)

 - 0.50.8
 - Updated test with linuxserver alpine 3.22, include file/magic (#3345)
 - Ensure a default Locale is set for more reliable text decoding (en_US.UTF-8 by default) (#3340)
 - Re #3337 - UI - Various fixes for 'Extract Data' (#3341)
 - UI - Fixing UI - Favicons - Turning off favicons misaligns other icons on lister page #3321

 - 0.50.7
 - UI - Set default favicon, handle default 'not set' for new/updated installations
 - UI - Set default favicon, offer option to disable favicons (#3316)
 - README - Updating screenshot (with better cropping)
 - README - Updating screenshot
 - UI - Mobile CSS tweaks
 - UI - Mobile - Small tidyups for mobile use
 - UI - CSS - Modernising stylesheet build

 - 0.50.6
 - Favicon type detection - support for autodetecting mimetype for better reliability (#3308)
 - Fixing ARMv7 docker image support for older devices (#3311)
 - UI - Favicons - Try /favicon.ico if no other was specified in the document
 - UI - Favicons - Realtime mode - Fixing small bug when favicon needed updating in realtime
 - UI - Favicons in realtime update mode, update after favicon was written to disk only.
 - UI - Lazy load favicons so it doesnt block realtime and other operations
 - UI - Adding Favicon support to watch overview lister page + FavIcon API (#3196)
 - UI - Sort list by Running or Paused #3284 (#3294)
 - Similarity condition - Skip generating stats for very large documents in the 'Edit' page (#3296)
 - Refactor watch history/diff page time handling, fixing issue where the last time viewed was not set in the 'history' page automatically (#3293)
 - Update stock-not-in-stock.js Added 'backorder' and 'more on order'
 - Update README.md

 - 0.50.5
 - Update README-pip.md
 - Update README.md
 - Update README.md
 - Data save - Solving JSON DB saving bug (#3286 #3260 #3259)
 - Conditions - Fixing "Does NOT contain" condition (#3279 / #3272 )
 - Update README.md
 - Update LICENSE

 - 0.50.4
 - CVE-2025-52558 - Fixing XSS in error handling output of watch overview list
 - Better path cross-platform file handling (#3265)

 - 0.50.3
 - Realtime UI - Prefer websocket then fallback to 'polling' mode, increase reconnecting retries.
 - UI - Fixing Watch 'set viewed' by tag #3253 (#3258)
 - UI - Always unset 'unviewed' state when '[History]' button  is pressed from watch overview list #3243
 - UI - Tweak UI option text description for 'Open history page in new tab' setting
 - UI/Application listening on IPv6 - Please use `-h ::` to listen on all IPv6 interfaces, `-p` is removed (#3257)
 - Realtime UI - Delete watch should update in realtime ( #3255 )
 - UI - Quick watch add form color fix
 - Application via HTTPS support -  Adding SSL setup and automated test (#3247) (#3252)
 - Browser Steps - Fix for `source:` URLs fix (#3254)
 - UI - Restyle of "quick watch add form" above watchlist
 - UI - Don't restrict page content box to 80% width (#3251)
 - UI - #3236 fix duplicate icon in watchlist
 - Data store - use original formatted data write
 - Realtime UI - Ability to notify browser/client if there was a notification event (#3235)
 - UI - Realtime - Add realtime warning to page if server goes offline
 - Browser Steps - Better support for sites that redirect on click/login etc
 - Restock detector - Update texts (#3234)
 - Puppeteer fetcher - Issue a Page.sendStop on frame load incase the browser is waiting for other data (#3232)
 - Build test - Build test for platforms in parallel (#3229)
 - BrowserSteps - remove unsupported exception class

 - 0.50.2
 - 0.50.1


 - 0.50.01
 - UI - Adding missing icons lib
 - Use pip build cache from inside Dockerfile (#3228)
 - UI - Also uncheck 'check all' checkbox for group operations in realtime mode
 - UI - Real time - checkbox operations now realtime without reload
 - UI - Revert icon changes
 - Building - Use GHA layer caching (#3227)
 - UI - Realtime - Fixing 'last_changed' status re #3224
 - Realtime UI - Socketio tweaks and refactor (#3220)
 - Code - Fix dep warning (#3221)
 - Realtime UI - Reducing log output
 - UI - Reword restock detector plugin description
 - UI - Remove incorrect error text

 - 0.49.18
 - Realtime UI updates via WebSocket (#3183)
 - Update to Apprise 1.9.3 - BlueSky, Resend support (#3216)
 - UI - Update 'Browser Steps' UI text
 - Code - Remove unused f-strings (#3209)
 - Use logger.debug for playwright console logs (#3201)

 - 0.49.17
 - Resolve warnings of bs4 library (#3187)
 - Revert memory strategy change for html_to_text (Was hanging under high concurrency setups)

 - 0.49.16
 - Fixes to ensure proxy errors are handled correctly (#3168)
 - UI - Custom headers should have validation (#3172)
 - Update selenium library (#3170)
 - Restock detection - adding new string
 - Conditions - Levenshtein text similarity plugin - adding test, fixing import, fixing check for watches with 1 snapshot history (#3161)
 - Restock detection - Use cleaner logic for limiting elements to scan, refactor, improve tests (#3158)
 - pyppeteer fast puppeteer fetch - be sure viewport is set to --window-size if --window-size is set (#3157)
 - Improved global ignore test (#3140)
 - Update docker-compose.yml (#3149)
 - Small fix for xpath element scraper (#3145)
 - Plugins for conditions (and include Similarity / Levenshtein, wordcount conditions) Re #3108
 - Browser Steps - <Select> by Option Text - #1224, #1228 (#3138)
 - Browser Steps - error reporting and session shutdown improvements (#3137)

 - 0.49.15
 - Visual Selector & Browser Steps - Always recheck if the data/screenshot is ready under "Visual Selector" tab after using Browser Steps (#3130)
 - App logs - Send TRACE and INFO logs to stdout (#3051)
 - Development: introduce Ruff as linter/formatter (#3039)
 - Updating restock texts (#3124)
 - Only add screenshot warning if capture was greater than trim size (#3123)

 - 0.49.14
 - Small fix for multiprocessing start on Mac OS (#3121 #3115)
 - docs: Update reference URL (#3119)
 - UI - Fix to edit and groups template
 - Updating API documentation
 - Undo forced selenium headless mode, small refactor (#3112)
 - Playwright + Puppeteer fix for when page is taller than viewport but less than screenshot step_size (#3113)
 - Memory management -  Run HTML to text in sub process, a few more cleanups  to playwright (#3110)
 - UI Edit/Stats - Add levenshtein distance info, explains how "different" the last two snapshot are (#3109)

 - 0.49.13
 - API - Added notifications API endpoints (#3103)
 - Fetcher - Use bigger screenshot chunks to speed up page screenshot (#3107)
 - App memory - Apprise import only when needed - saves ~50Mb RAM if you dont have any notifications enabled (#3106)
 - Fetching - Small improvement memory handling in detecting price information (saves ~10Mb)
 - Refactor image saving with forked process to reduce memory usage, improvements to xpath scraper handling (#3099)
 - Update other methods to use updated screenshot handler (#3098)
 - Memory fixes for large playwright screenshots (#3092)
 - Filters - Support multi line regex  (#2889)
 - UI - Add UI options tab and setting to disable opening diff in a new tab (#3071)
 - README.md update - Including blurb about 'conditions'
 - Requests fetcher - Remove old screenshot when watch was in a different fetcher type (#3097)
 - Make chrome browser headless when checking the site with selenium (#3095)
 - UI - Field name update - Keyword triggers - Trigger/wait for text (#3088)
 - UI - "Recheck all" should also queue most overdue first  (same like automatic scheduler) (#3087)
 - Groups - Including "Extract text", "Text to ignore", "Trigger text" and "Text that should not be present" filters

 - 0.49.12

 - 0.49.11

 - 0.49.10
 - Update README.md
 - UI - "Conditions" section, making the Conditions setup table work better on mobile/responsive
 - Adding a GC memory cleanup (releases cached libxml memory and others) (#3079)
 - Python 3.11 container base (#3077)
 - Use lowercase static asset filenames
 - Restock detection - Add Indonesian phrases for out-of-stock detection (#3075)
 - Regession - Shared history/diff page with anonymous access turned on should allow screenshot access (#3076)
 - Update edit.html - linking to tutorial
 - Code - Tidy up lint errors (#3074)
 - UI - Update edit.html- xPath support text for 1 & 2
 - Text/fetching - Small fix for when last fetched was zero bytes and special options (removals/additions/changes) was set (#3065)
 - Notifications backend - Refactor + tests for Apprise custom integration (#3057)
 - UI - Watch edit - "Clone" Should be "Clone & Edit" without watch history, redirect to the new edit page (#3063 #2782)
 - UI - Conditions - Offer some information about what the filter/condition/trigger saw (#3062)
 - UI - Tidy up support links
 - UI - Set a graph % of ETA time completed of checking the watch (#3060)

 - 0.49.9
 - RSS Fixes and improvements - Ability to set "RSS Color HTML Format" in Settings, detect and filter content with bad content that could break RSS (#3055)

 - 0.49.8
 - Server - Path blueprint fixes and moving code blueprint to fix RSS forward slash on url (#3054)
 - API - Adding "Search" API (#3052)
 - Fetching - Upgrading to pyppeteer-ng 2.0.0rc8 (more modern pyee requirements)

 - 0.49.7
 - Adding Tags/Groups API (#3049)

 - 0.49.6
 - API Access should still work even when UI Password is enabled (#3046) #3045

 - 0.49.5
 - Template tidyup & UI Fixes (#3044)
 - Watch history -  Ensure atomic/safe history data disk writes (#3042 #3041)
 - Testing - Replace Linux only 'resource' library with cross-platform 'psutil' library (#3037)
 - Refactor code layout, add extra tests
 - New major functionality CONDITIONS - Compare values, check numbers within range, etc

 - 0.49.4
 - Datastore - Always use utf-8 encoding for error text output storage
 -  Restock detection - Adding french keywords for out of stock items
 - Browser Steps - Should use the Watch URL/link after any Jinja2 type templates are applied
 - BrowserSteps - Speed up scraping, refactor screenshot handling for very long pages (#2999)
 - Browser Steps - Added new "Make all child elements visible" action
 - Browser Steps - Added new "Remove elements" action
 - UI - Browser Steps - "Click X,Y" should focus on the input field also
 - UI - Browser Steps - Improving Browser Steps usability on mobile

 - 0.49.3
 - UI - Reverting JS change to tabs (the better fix was the W3C HTML validation)

 - 0.49.2
 - UI - Make the setup and error messages for Visual Selector and Browser Steps a lot more meaningful (#2977)
 - Update docker-compose.yml
 - UI - More W3C HTML validation fixes
 - UI - More W3C validation fixes (#2973)
 - UI - Tweaks for HTML validation
 - Filter - "Unique lines" could possibly crash if history was empty or cleared on the disk
 - UI - Sometimes the DOM wasnt ready when tab selection triggered via CSS, which displayed empty tabs on some browsers
 - Removing deprecated docker-compose.yml version attribute (#2967)
 - Update settings.html
 - Browser Steps - Increasing timeout for actions and unifying timeout values
 - Browser Steps - Fixing 'Uncheck checkbox' #2958
 - UI - "Browser Steps" tab should be always available with helpful info (evenwhen playwright is not configured) (#2955)
 - Adding `browser_steps` JSON Schema rule for API updates (#2957)
 - UI - Fix mute/unmute alt/title label alt/title text in watch overview (#2951)

 - 0.49.1
 - Update stock-not-in-stock.js - Italian (#2948)
 - Re #2945 - Handle/Strip UTF-8 ByteOrderMark in JSON strings correctly (fixes `"Exception: No parsable JSON found in this document" ` error) (#2947)
 - Add major and minor tags for Docker release workflow (#2938)
 - Adding jinja2/browsersteps test (#2915)
 - Header handling - Fix header parsing to split on the first colon only (headers where the value contained :// type may have been broken) (#2929)

 - 0.49.00
 - Update README.md
 - Build/Libraries - Pin `referencing` library which breaks due to out-dated flask_expects_json, remove pip upgrade in test(#2912)
 - Notifications - Custom POST:// GET:// etc endpoints - returning 204 and other 20x responses are OK (don't show an error was detected)(#2897)

 - 0.48.06
 - Restock -  Add test for new lower/higher price notification Re #2715 (#2892)
 - Update integration test for "linuxserver" test build (#2891)
 - Notifications - Update Apprise to 1.9.2 - Fixes custom posts:// gets:// etc URL's being double-encoded, fixes chantify:// notifications (#2868) (#2875)  (#2870)
 - Custom posts:// get:// notifications etc - Be sure our custom extensions are imported (#2890)
 - "Send test notification" button - Easier to understand test send results, Improved error handling, code refactor (#2888)
 - Improve `last_checked` vs `last_changed` time information precision (#2883)
 - Update Apprise to 1.9.1 (#2876)
 - Builder/Docker - Remove PUID and PGID ( they were not used ) (#2852)
 - UI - Fix diff not starting from last viewed snapshot (#2744) (#2856)

 - 0.48.05
 - Fixing test for CVE-2024-56509 (#2864)
 - CVE-2024-56509 - Stricter file protocol checking pre-check ( Improper Input Validation Leading to LFR/Path Traversal when fetching file:.. )

 - 0.48.04
 - Windows was sometimes missing timezone data (#2845 #2826)

 - 0.48.03
 - 0.48.02
 - 0.48.02
 - Notifications - "Send test" was not always following "System default notification format" (#2844)
 - Notifications - "Send test" was not always following "System default notification format"

 - 0.48.02
 - Notifications - "Send test" was not always following "System default notification format"
 - Notifications - Default notification format (for new installs) now "HTML color" (#2843)
 - Notification - `HTML Color` format notification colors should be same as UI, `{{diff_full}}` token should also get HTML colors ( #2842 #2554 )
 - Notifcations - Adding "HTML Color" notification format option (#2837)
 - UI - Make 'tag' sticky - redirect to current tag on edit or add watch (#2824 #2785)
 - Notifications - Support for commented out notification URLs (#2825 #2769)
 - Docs - Adding information to README.md about the new scheduler

 - 0.48.01
 - UI - Fixing scheduler options

 - 0.48.00
 - Fix HIDE_REFERER env option for hiding changedetection.io from referer headers (#2787)
 - New functionality - Time (weekday + time) scheduler / duration (#2802)
 - Add Turkish phrases for out-of-stock detection (#2809)
 - UI - Always use UTC timezone for storing data, show local timezone (#2799)
 - Update stock-not-in-stock.js
 - Python 3.13 compatibility (#2791)
 - Code - Update .gitignore and .dockerignore (#2797)
 - VisualSelector - Use 'deflate' for storing elements.json, 90% file size reduction (#2794)
 - UI - Show local timezone info in settings (for future functionality) #2793
 - Notification - Locking paho-mqtt:// version fix
 - Update COMMERCIAL_LICENCE.md
 - Ability to disable version check (set `DISABLE_VERSION_CHECK=true`) Re #2773 (#2775)
 - Minor improvement for queue management
 - Update bug_report.md

 - Security - Fix test
 - Security check - improve test
 - 0.47.06
 - CVE-2024-51998 - file:/ path traversal access should not be allowed to access a file without ALLOW_FILE_URI set
 - Update docker-compose.yml (#2767)
 - Price tracker - fix for sites that supply an empty additional price (#2758)
 - Testing - Pinning werkzeug (#2757)

 - 0.47.05
 - CVE-2024-51483 - Fix for limiting access to file:// via source:file:///tmp/file.txt when using webdriver/playwright
 - Backups - Hide incomplete/running backups from being downloaded
 - Backups - Backups now operate in the background, provide a nice UI to access/download previous backups (#2755)
 - Filters - Process all CSS and XPath 'subtract' selectors in a single pass to prevent index shifting and reference loss during DOM manipulation. (#2754)

 - 0.47.04
 - Do not recheck 'paused' watches on edit/save (Re #2747 #2750)
 - Notification post:// get:// etc - Fixing URL encoding of headers so that '+' in 

[Content truncated]
```
