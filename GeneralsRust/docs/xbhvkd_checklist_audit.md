# hq-xbhvkd checklist audit

Head: `551a6fd762f275ffe50e473e585df52ff057901e`

## Method

1. Parse survivor checklist lines (`id — title — C++ ref`).
2. Load each child bead body; extract `THE_*` / `PROCESS_*` / `*_STATE` / ambient phrases.
3. `rg -w` under `GeneralsRust/Code/**/*.rs`, ignoring `//` comment-only hits.
4. **CLEAR** when all named ownership symbols are absent, or title is not a
   process/TLS/ambient ownership residual (theme mismatch for this survivor).
5. **KEEP** when a named ambient still exists in code, or the item is
   `hq-lfke6` / `hq-spb88j`, or the title names process/ambient/GlobalData/EngineStores/WorldServices.

## Counts

- Cleared: 185
- Remaining: 28

## Remaining IDs

- hq-0kgryf
- hq-41lmo
- hq-4hecb
- hq-6oqj89
- hq-b5493l
- hq-gy8agp
- hq-i1rry0
- hq-irfja2
- hq-is7dx6
- hq-kztrtz
- hq-leu1qg
- hq-lfke6
- hq-lqfh3
- hq-miy9q
- hq-n6keim
- hq-npnlu
- hq-nykmiy
- hq-o5coj7
- hq-r1fp6
- hq-rm2do
- hq-rxmn7u
- hq-spb88j
- hq-u3nwun
- hq-u7fj3
- hq-uhn0i0
- hq-x1j4u.1.8
- hq-xagq7
- hq-z595v6
