# EQ assistant evaluation

There is no public benchmark of an LLM's "EQ skill". HomeFi doesn't need one:
the acoustics come from AutoEq measurements and from the app's own checks
(boost caps, automatic preamp, the small-speaker rule). What the model has to
get right is the conversation: calling tools reliably, writing good Spanish and
English, making a change the listener can actually hear (and then calibrating
it), and staying cheap. The prompts include a frequency vocabulary, starting
points per genre, a magnitude ladder by intensity (Subtle / Balanced / Bold) and
a short table of plain phrases with the change each one usually needs at the
Balanced intensity ("le falta bajo" → +3..4 dB low shelf, "boomy" → −3 dB around
80–200 Hz...).
A Jan-2026 Aalborg / B&O paper (arXiv 2601.09448) found that in-context
examples like these align text with EQ better than rules alone.

`crates/homefi-core/examples/ai_eval.rs` checks how a provider and model handle
that conversation. It is **opt-in and never runs in CI**: it calls the
provider's API and spends tokens (roughly 11 conversations of 5 to 10 calls each).

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

Eleven scripted listeners, Spanish and English: Sony WH-1000XM5 over Bluetooth
(reggaetón, wants more bass), MacBook speakers (podcasts), Sennheiser HD 650 on
a USB DAC (jazz, neutral), a car stereo (rock, noisy), AirPods Pro 2 (pop,
harsh), a phone speaker (hip-hop, thin), a Beyerdynamic DT 770 Pro on a DAC with
a Bluetooth speaker also connected (classical, dull) and Galaxy Buds2 Pro
(house, boomy), a nu-metal fan (System of a Down) on an HD 650 + DAC who says "que pegue más" and
should get 2 to 4 bands with a main move of at least 3 dB, plus two taste-curve listeners: one opens the assistant to refine an existing
"More punch for David Guetta" layer (cymbals too harsh, hissing s's) and one wants a layer for an
artist on a phone speaker. Each answers the assistant's questions with its scripted
replies in order ("I don't know, you decide" when they run out) and says yes
to the first proposal.

## Scores

| Column | Passes when |
| --- | --- |
| `proposal` | A profile or a taste layer was proposed within 9 user replies (the turn is shown) |
| `autoeq` | The headphone is in AutoEq and the model fetched its profile (`-` when there is none) |
| `caps` | The model's own numbers respect the caps: taste within ±6 dB, profile bands at most +12 dB, and on built-in speakers no bass above +3 dB nor any boost below 150 Hz. The app clamps these anyway; this measures whether the model follows the rules |
| `curves` | Every taste curve the model sent (`propose_taste_rule` / `set_taste_rule` with `bands`) has valid bands (known kind, ±9 dB, peaks Q 0.3-8, a note, no boost below 150 Hz on built-in speakers), came after an `analyze_sound`, and has no two bands of one kind within a third of an octave (stacking). The refine persona must design at least one curve; `-` when none was asked for |
| `audible` | The biggest move the model asked for (a band, a profile band or a tilt knob, before the app clamps it) reaches the smallest main move of the Balanced intensity, 3 dB (`-` for the persona that asks for a neutral sound). The nu-metal persona must also send 2 to 4 bands (a tilt knob counts as one) |
| `<=6 q` | At most 6 questions before the proposal |
| `lang` | Every message is in the user's language (a word-count guess) |
| `claims` | No "studio quality" / "calidad de estudio" / "perfect sound" promises |
| `applied` | The model called `apply_eq` (or saved the taste layer) after the "yes" |

The last line adds up every check. Compare models by running the same command
with a different `HOMEFI_EVAL_MODEL`. Results vary from run to run, so run it a
couple of times before drawing conclusions.
