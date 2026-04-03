# Michael Jackson Vocal Technique Analysis

Reference document for how MJ's signature vocal techniques relate to beat and key detection.

## Signature Vocal Techniques

| Technique | Frequency | Peak Albums | Signature Song |
|---|---|---|---|
| Vocal hiccup (glottal stop) | Very High | Off the Wall, Thriller | "Don't Stop 'Til You Get Enough" |
| Breathy gasps | Very High | All albums | "Bad" |
| "Hee-hee" | High | Bad, Dangerous | "Bad", "Thriller" |
| "Ow!" shouts | High | Bad, Dangerous | "Bad", "Smooth Criminal" |
| "Shamone" | Moderate | Bad, Dangerous | "Bad" |
| Beatboxing / vocal percussion | Moderate | Dangerous, HIStory | "Dangerous" |
| Falsetto passages | Very High | Off the Wall | "Don't Stop 'Til You Get Enough" |
| Grunts / moans | High | Dangerous | "In the Closet" |
| Emotional cry / break | Low | Off the Wall, HIStory | "She's Out of My Life" |

Style evolved from lighter, falsetto-dominant (Off the Wall) toward more aggressive, chest-voice-dominant with heavier grunts (Bad, Dangerous).

---

## Beat Placement of Ad-Libs

### Per-technique rhythmic position

- **"Hee-hee"**: Upbeats (the "&" of beats 2 and 4). Almost never on strong beats. Functions like a hi-hat accent.
- **"Ow!" / shouts**: Downbeats (beat 1 or 3). Strong-beat marker useful for downbeat estimation.
- **Gasps / breaths**: The "&" of beat 4, anticipating the next downbeat by ~200-300ms. Pre-downbeat predictors.
- **Hiccups**: 16th-note subdivisions ("e" or "a" positions). Indicate subdivision density.
- **Grunts**: Beats 2 and 4 (backbeat), doubling the snare function.

### Tempo associations

| BPM Range | Ad-libs/min | Character | Dominant techniques |
|---|---|---|---|
| Below 90 (ballads) | < 2 | Expressive, not rhythmic | Breaths, melodic ad-libs |
| 90-110 (mid-tempo) | 5-15 | Widest variety | Hiccups, hee-hee, gasps |
| Above 110 (uptempo) | 15-40+ | Primarily percussive | Hee-hee, grunts, shouts |

### Song structure escalation

Ad-lib density consistently increases through the song:

```
Intro:  [minimal]
Verse:  [low]
Chorus: [high]
Verse2: [low-moderate]
Chorus: [high]
Bridge: [variable]
Chorus: [higher]
Outro:  [maximum]
```

Each successive chorus has more ad-libs than the previous one. High density suggests the track is past the midpoint approaching the outro.

### Interaction with drum patterns

- Complementary placement: when kick hits beats 1 and 3, ad-libs fill beats 2/4 or upbeats.
- In percussion breakdowns, ad-libs increase in density to compensate.
- Programmed tracks ("Dangerous", "Jam"): ad-libs align to 16th-note grid.
- Live-feel tracks ("Don't Stop 'Til You Get Enough"): ad-libs have 10-40ms behind-the-grid swing.

---

## Key and Harmony

### Preferred keys

~65% of his catalog uses flat keys: Eb, Ab, Bb, Db, F minor, Bb minor.

**By album era:**
- Quincy Jones era (Off the Wall - Bad): Eb, Ab, Bb centers
- Teddy Riley era (Dangerous): More E minor, D minor mixed with flat keys
- HIStory: Darker minor keys (C# minor, F minor, Bb minor)

### Song key catalog

**Off the Wall (1979)**

| Song | Key |
|---|---|
| Don't Stop 'Til You Get Enough | B major |
| Rock with You | Eb major |
| Off the Wall | Eb major |
| Working Day and Night | F minor |
| She's Out of My Life | D major (-> Eb) |

**Thriller (1982)**

| Song | Key |
|---|---|
| Wanna Be Startin' Somethin' | E minor |
| Baby Be Mine | Bb minor |
| The Girl Is Mine | F major |
| Thriller | C# minor |
| Beat It | Eb minor |
| Billie Jean | F# minor |
| Human Nature | G major |
| P.Y.T. | A major |
| The Lady in My Life | Bb major |

**Bad (1987)**

| Song | Key |
|---|---|
| Bad | A minor |
| The Way You Make Me Feel | Eb major |
| Speed Demon | G major |
| Dirty Diana | G minor |
| Smooth Criminal | A minor |
| Man in the Mirror | G major (-> Ab -> A) |
| Another Part of Me | Ab major |
| Liberian Girl | Db major |

**Dangerous (1991)**

| Song | Key |
|---|---|
| Jam | D minor |
| Why You Wanna Trip on Me | E minor |
| In the Closet | F minor |
| Remember the Time | Ab major |
| Black or White | E major (-> A) |
| Who Is It | F# minor |
| Heal the World | Ab major (-> A) |
| Will You Be There | Bb major |
| Dangerous | G# minor |

**HIStory (1995)**

| Song | Key |
|---|---|
| Scream | C# minor |
| They Don't Care About Us | Bb minor |
| Stranger in Moscow | F minor |
| Earth Song | Ab minor (-> A minor -> Bb minor) |
| You Are Not Alone | E major (-> F) |
| Childhood | Bb major |
| Tabloid Junkie | F minor |
| D.S. | E minor |

### Vocal range

- Full studio range: Eb2 to Bb5 (~3.5 octaves)
- Chest voice: Eb2 to Eb4
- Mixed/belt: Eb4 to Ab4
- Falsetto: Ab4 to Bb5
- Sweet spot: F3-F4 chest, Ab4-Eb5 falsetto

### Ad-lib pitch relationships

| Ad-lib | Pitched? | Typical scale degrees | Notes |
|---|---|---|---|
| "Hee-hee" | Yes (falsetto) | Root or 5th | Confirms key center, Bb4-Eb5 range |
| "Aaow" / "Ooh" | Yes | Root, 5th, or b7 | Melodic ornaments |
| "Ow!" | Semi-pitched | Root or b7 | Broadband attack transient, pitched tail |
| Hiccups | Semi-pitched | 1, 3, or 5 | Very short (50-100ms segments) |
| Grunts / beatboxing | Unpitched | N/A | Exclude from chromagram |
| Breaths | Unpitched | N/A | Exclude from chromagram |

Common melodic intervals in ad-libs:
- Minor 3rd down (blues/gospel reflex, e.g. "hee-HEE")
- Perfect 4th up (pickup-style "ow-OW")
- Whole/half step (chromatic approach in "shamone" figures)

### Register break as key indicator

Passaggio at Eb4-F4. Keys were chosen so the chest-to-falsetto transition lands on a harmonically meaningful pitch (tonic, 4th, or 5th). Detecting the register flip frequency can cross-check estimated key.

| Song Key | Chest Ceiling | Falsetto Entry | Scale Degree at Break |
|---|---|---|---|
| Eb major | Eb4 (tonic) | Ab4-Bb4 (4th-5th) | Tonic |
| F# minor | F#4 (tonic) | A4-B4 (m3-4th) | Tonic |
| A minor | E4-F4 (5th-b6) | A4 (tonic up) | 5th |
| Ab major | Eb4 (5th) | Ab4 (tonic up) | 5th |
| G major | D4-E4 (5th-6th) | G4-A4 (tonic-2nd) | 5th |

### Modulation patterns

~25-30% of singles use a key change. The pattern is very consistent:
- Direction: almost exclusively **upward**
- Interval: overwhelmingly **half step** (minor 2nd). Occasionally whole step.
- Timing: **final 20-30%** of the song (last chorus or coda)
- Mechanism: **direct shift** (no pivot chord) -- clear discontinuity in chroma vector

Notable examples:
- Man in the Mirror: G -> Ab -> A (two half steps)
- Earth Song: Ab minor -> A minor -> Bb minor (two half steps)
- You Are Not Alone: E -> F
- Heal the World: Ab -> A
- She's Out of My Life: D -> Eb
