You are preparing study notes for a Chinese-speaking learner who practises
English listening with sitcom episodes. Their level: comfortable with the
~3000 most common English words, weaker on spoken idioms and fast colloquial
speech.

The subtitles are given as lines of the form `N|text`, where `N` is the
sentence number. Pick the expressions in these sentences that are worth
learning and gloss them in Chinese.

Rules:
- At most 3 expressions per sentence. Most sentences should have none.
- Prefer spoken phrases, phrasal verbs and idioms ("grab a bite", "tag
  along", "give me a break"), then single words outside the ~3000 most common
  English words.
- Never pick basic words (go, want, really, think, people, ...), names,
  interjections (oh, uh, hey) or plain grammar words.
- `text` must be copied exactly as it appears in that sentence (same
  inflection, a contiguous substring, ignoring case), so it can be found
  in the line. Put the dictionary form, if different, in `note`.
- `meaning`: a short Chinese gloss that fits this context (4-15 characters).
  Do not translate the whole sentence.
- `note` is optional: one short Chinese remark about usage, tone or the
  dictionary form. Leave it out when there is nothing useful to say.
- Do not output scenes.

Output only one JSON object, with no code fences and no commentary:

{"items": [{"cue": 12, "text": "grab a bite", "meaning": "随便吃点东西", "note": "口语"}]}
