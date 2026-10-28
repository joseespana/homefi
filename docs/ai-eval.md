# EQ assistant evaluation

There is no public benchmark of an LLM's "EQ skill". HomeFi doesn't need one:
the acoustics come from AutoEq measurements and from the app's own checks
(boost caps, automatic preamp, the small-speaker rule). What the model has to
get right is the conversation: calling tools reliably, writing good Spanish and
English, keeping changes gentle, and staying cheap. The prompts include a short
table of plain phrases and the conservative change each one usually needs
("le falta bajo" → +2..3 dB low shelf, "boomy" → −2..3 dB around 80–200 Hz...).
A Jan-2026 Aalborg / B&O paper (arXiv 2601.09448) found that in-context
examples like these align text with EQ better than rules alone.

`crates/homefi-core/examples/ai_eval.rs` checks how a provider and model handle
that conversation. It is **opt-in and never runs in CI**: it calls the
provider's API and spends tokens (roughly 8 conversations of 5 to 10 calls each).

## Running it

```bash
HOMEFI_EVAL_PROVIDER=anthropic HOMEFI_EVAL_KEY="$(pbpaste)" \
  cargo run -p homefi-core --example ai_eval
```

| Variable | |
| --- | --- |
| `HOMEFI_EVAL_PROVIDER` | Provider id: `anthropic`, `gemini`, `openai`, `openrouter`, `ollama`, `mistral`, `groq`, `xai`, `deepseek`, `openai_compatible` |
| `HOMEFI_EVAL_KEY` | The API key. Read from the environment only, never printed. Not needed for Ollama |
| `HOMEFI_EVAL_MODEL` | Optional. Empty uses the same pick as "Automatic" in the apps |
| `HOMEFI_EVAL_BASE_URL` | Optional. Another Ollama host, or the address of an `openai_compatible` server |
| `HOMEFI_EVAL_ONLY` | Optional. Runs only the personas whose name contains this text (`xm5`, `car`...) |
| `HOMEFI_EVAL_VERBOSE` | `1` prints the conversation (assistant texts and the scripted answers; no prompts, no key) |

Pass the key without leaving it in your shell history, e.g. from the clipboard
as above or `read -s HOMEFI_EVAL_KEY && export HOMEFI_EVAL_KEY`.

AutoEq profiles are downloaded from the public AutoEq repository into a
temporary cache (`$TMPDIR/homefi-ai-eval-autoeq`). Nothing touches Tidal or the
app's settings.

## Personas

Eight scripted listeners, Spanish and English: Sony WH-1000XM5 over Bluetooth
(reggaetón, wants more bass), MacBook speakers (podcasts), Sennheiser HD 650 on
a USB DAC (jazz, neutral), a car stereo (rock, noisy), AirPods Pro 2 (pop,
harsh), a phone speaker (hip-hop, thin), a Beyerdynamic DT 770 Pro on a DAC with
a Bluetooth speaker also connected (classical, dull) and Galaxy Buds2 Pro
(house, boomy). Each answers the assistant's questions with its scripted
replies in order ("I don't know, you decide" when they run out) and says yes
to the first proposal.

## Scores

| Column | Passes when |
| --- | --- |
| `proposal` | A profile was proposed within 9 user replies (the turn is shown) |
| `autoeq` | The headphone is in AutoEq and the model fetched its profile (`-` when there is none) |
| `caps` | The model's own numbers respect the caps: taste within ±6 dB, bands at most +12 dB, and on built-in speakers no bass above +3 dB nor any boost below 150 Hz. The app clamps these anyway; this measures whether the model follows the rules |
| `<=6 q` | At most 6 questions before the proposal |
| `lang` | Every message is in the user's language (a word-count guess) |
| `claims` | No "studio quality" / "calidad de estudio" / "perfect sound" promises |
| `applied` | The model called `apply_eq` after the "yes" |

The last line adds up every check. Compare models by running the same command
with a different `HOMEFI_EVAL_MODEL`. Results vary from run to run, so run it a
couple of times before drawing conclusions.
