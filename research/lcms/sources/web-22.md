# Web source

- URL: https://conceptnet.io
- Title: # ConceptNet
- Captured (UTC): 2026-06-29T16:30:07.859226098+00:00

```text
# ConceptNet

## An open, multilingual knowledge graph
* [Documentation][1]
* [FAQ][2]
* [Chat][3]
* [Blog][4]
* [Documentation][5]
* [FAQ][6]
* [Chat][7]
* [Blog][8]

English Chinese Dutch French German Italian Japanese Portuguese Russian Spanish Afrikaans Albanian Ancient Greek Arabic
Armenian Aromanian Asturian Azerbaijani Basque Belarusian Bulgarian Catalan Chinese Classical Armenian Czech Danish
Dutch English Esperanto Estonian Faroese Filipino Finnish French Galician Georgian German Greek Hebrew Hindi Hungarian
Icelandic Ido Indonesian Irish Italian Japanese Jèrriais Kazakh Korean Kurdish Latin Latvian Lithuanian Macedonian
Malagasy Malay Manx Navajo Northern Sami Norwegian Occitan Old English Old French Old Norse Persian Polish Portuguese
Romanian Russian Sanskrit Scottish Gaelic Serbo-Croatian Slovak Slovenian Spanish Swahili Swedish Tamil Telugu Thai
Turkish Ukrainian Upper Sorbian Urdu Vietnamese Volapük Welsh
Search

## What is ConceptNet?

**ConceptNet** is a freely-available semantic network, designed to help computers understand the meanings of words that
people use.

ConceptNet originated from the crowdsourcing project Open Mind Common Sense, which was launched in 1999 at the MIT Media
Lab. It has since grown to include knowledge from other crowdsourced resources, expert-created resources, and games with
a purpose.

## Examples

To explore what's in ConceptNet, try browsing what it knows about any of these terms:
* en [word][9]
* fr [mot][10]
* nl [woord][11]
* es [palabra][12]
* pt [palavra][13]
* ja [単語][14]
* en [graph][15]
* en [knowledge][16]
* en [learn][17]
* en [natural language][18]
* en [semantic network][19]
* mul [💡][20]

## Word vectors and recent publications

ConceptNet is used to create [word embeddings][21] -- representations of word meanings as vectors, similar to word2vec,
GloVe, or fastText, but better.

These word embeddings are free, multilingual, aligned across languages, and designed to avoid representing harmful
stereotypes. Their performance at word similarity, within and across languages, was shown to be state of the art at
[SemEval 2017][22].

The process for learning these word vectors is described in our [AAAI 2017][23] paper, which also shows state-of-the-art
results on solving analogy problems.

## Support and discussion

[ [chat on Gitter] ][24]

Detailed documentation about ConceptNet appears on its [GitHub wiki][25].

You can chat with ConceptNet developers and users on [Gitter][26], or join the [conceptnet-users mailing list][27].

Updates to ConceptNet and its supporting technologies appear on the [ConceptNet blog][28].

## Linked Open Data API

{
  "@id": "[/a/[/r/UsedFor/,/c/en/example/,/c/en/explain/]][29]",
  "dataset": "[/d/conceptnet/4/en][30]",
  "end": {
    "@id": "[/c/en/explain][31]",
    "label": "explain something",
    "language": "en",
    "term": "[/c/en/explain][32]"
  },
  "license": "[cc:by/4.0][33]",
  "rel": {
    "@id": "[/r/UsedFor][34]",
    "label": "UsedFor"
  },
  "sources": [
    {
      "activity": "[/s/activity/omcs/omcs1_possibly_free_text][35]",
      "contributor": "[/s/contributor/omcs/pavlos][36]"
    }
  ],
  "start": {
    "@id": "[/c/en/example][37]",
    "label": "an example",
    "language": "en",
    "term": "[/c/en/example][38]"
  },
  "surfaceText": "You can use [[an example]] to [[explain something]]",
  "weight": 1.0,
  "@context": [
    "[//api.conceptnet.io/ld/conceptnet5.7/context.ld.json][39]",
    "[//api.conceptnet.io/ld/conceptnet5.7/pagination.ld.json][40]"
  ]
}

ConceptNet is a proud part of the ecosystem of [Linked Open Data][41].

As a modern Linked Open Data resource, the data in ConceptNet is available in a [JSON-LD][42] API, a format that aims to
make linked data easy to understand and easy to work with. If you don't care what JSON-LD is, it's just a JSON REST API
with some extra metadata.

You can use [ExternalURL][43] links in ConceptNet to find the same terms in other vocabularies, such as WordNet,
DBPedia, and OpenCyc, which can provide you with other forms of information.

For information on how to use the ConceptNet API, see the [API documentation][44]. Or just [start browsing it][45] and
you'll probably figure it out.

## Sources of knowledge

Previous versions of ConceptNet were a home-grown crowd-sourced project, where we ran a Web site (Open Mind Common
Sense) collecting facts from people who came to the site. The Web of Data is much bigger than that now. Our data comes
from many different sources, some of which you can contribute to and improve not just the state of computational
knowledge, but of human knowledge.
* ConceptNet 5, like previous versions, contains the relational knowledge contributed to Open Mind Common Sense and its
  sister projects in other languages.
* We connect to a subset of [DBPedia][46], which extracts knowledge from the infoboxes on Wikipedia articles.
* Much of our knowledge comes from [Wiktionary][47], the free multilingual dictionary. This gives us information about
  synonyms, antonyms, translations of concepts into hundreds of languages, and multiple labeled word senses for many
  words.
* More dictionary-style knowledge comes from [Open Multilingual WordNet][48].
* We imported a high-level ontology from OpenCyc (by Cycorp, formerly hosted at cyc.com).
* Some knowledge about people's intuitive word associations comes from "games with a purpose". We have learned facts in
  English from Verbosity, a word game formerly run by the [ GWAP project][49], and in Japanese from the "nadya.jp" game
  by Nihon Unisys and Dentsu.

If you believe a term should be understood by ConceptNet, the most straightforward way to add it to a future build is to
add information about that term to [ Wiktionary][50], following their guidelines.

## Attributing ConceptNet

To give proper attribution to ConceptNet's data, we suggest this text:

> This work includes data from ConceptNet 5, which was compiled by the Commonsense Computing Initiative. ConceptNet 5 is
> freely available under the Creative Commons Attribution-ShareAlike license (CC BY SA 4.0) from
> [https://conceptnet.io][51]. The included data was created by contributors to Commonsense Computing projects,
> contributors to Wikimedia projects, Games with a Purpose, Princeton University's WordNet, DBPedia, OpenCyc, and Umbel.

A paper you can cite about ConceptNet is:

> Robyn Speer, Joshua Chin, and Catherine Havasi. 2017. "[ConceptNet 5.5: An Open Multilingual Graph of General
> Knowledge][52]." In proceedings of *AAAI* 31.

## Development

[Luminoso logo]

Development of ConceptNet takes place as an open-source project of [Luminoso Technologies, Inc.][53] The code that
builds and powers ConceptNet is available [on GitHub][54].

ConceptNet originated at the MIT Media Lab, and became part of the Commonsense Computing Initiative, a collaboration
between MIT and other labs and companies around the world. This global collaboration helps us collect relational
knowledge in many languages. The Commonsense Computing Initiative was founded by [Catherine Havasi][55], now the CEO of
Luminoso.

The development of ConceptNet 5 is led by Robyn Speer, a Luminoso co-founder, with contributions from [several other
people][56].

[ [Creative Commons License] ][57]
ConceptNet 5 is licensed under a [Creative Commons Attribution-ShareAlike 4.0 International License][58]. If you use it
in research, please cite [this AAAI paper][59].
See [Copying and Sharing ConceptNet][60] for more details.

[1]: https://github.com/commonsense/conceptnet5/wiki
[2]: https://github.com/commonsense/conceptnet5/wiki/FAQ
[3]: https://gitter.im/commonsense/conceptnet5
[4]: https://blog.conceptnet.io
[5]: https://github.com/commonsense/conceptnet5/wiki
[6]: https://github.com/commonsense/conceptnet5/wiki/FAQ
[7]: https://gitter.im/commonsense/conceptnet5
[8]: http://blog.conceptnet.io
[9]: /c/en/word
[10]: /c/fr/mot
[11]: /c/nl/woord
[12]: /c/es/palabra
[13]: /c/pt/palavra
[14]: /c/ja/単語
[15]: /c/en/graph
[16]: /c/en/knowledge
[17]: /c/en/knowledge
[18]: /c/en/natural_language
[19]: /c/en/semantic_network
[20]: /c/mul/%F0%9F%92%A1
[21]: https://github.com/commonsense/conceptnet-numberbatch
[22]: https://arxiv.org/abs/1704.03560
[23]: https://arxiv.org/abs/1612.03975
[24]: https://gitter.im/commonsense/conceptnet5?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge
[25]: https://github.com/commonsense/conceptnet5/wiki
[26]: https://gitter.im/commonsense/conceptnet5?utm_source=share-link&utm_medium=link&utm_campaign=share-link
[27]: https://groups.google.com/group/conceptnet-users
[28]: https://blog.conceptnet.io/
[29]: //api.conceptnet.io/a/[/r/UsedFor/,/c/en/example/,/c/en/explain/]
[30]: //api.conceptnet.io/d/conceptnet/4/en
[31]: //api.conceptnet.io/c/en/explain
[32]: //api.conceptnet.io/c/en/explain
[33]: https://creativecommons.org/licenses/by/4.0
[34]: //api.conceptnet.io/r/UsedFor
[35]: //api.conceptnet.io/s/activity/omcs/omcs1_possibly_free_text
[36]: //api.conceptnet.io/s/contributor/omcs/pavlos
[37]: //api.conceptnet.io/c/en/example
[38]: //api.conceptnet.io/c/en/example
[39]: //api.conceptnet.io/ld/conceptnet5.7/context.ld.json
[40]: //api.conceptnet.io/ld/conceptnet5.7/pagination.ld.json
[41]: http://linkeddata.org/
[42]: https://json-ld.org/
[43]: /r/ExternalURL
[44]: https://github.com/commonsense/conceptnet5/wiki/API
[45]: //api.conceptnet.io/c/en/example
[46]: https://dbpedia.org
[47]: https://en.wiktionary.org
[48]: http://compling.hss.ntu.edu.sg/omw/
[49]: https://www.cmu.edu/homepage/computing/2008/summer/games-with-a-purpose.shtml
[50]: https://en.wiktionary.org
[51]: //conceptnet.io
[52]: https://arxiv.org/abs/1612.03975
[53]: https://www.luminoso.com
[54]: https://github.com/commonsense/conceptnet5
[55]: https://www.linkedin.com/in/havasi
[56]: https://github.com/commonsense/conceptnet5/wiki/Copying-and-sharing-ConceptNet#credits-and-acknowledgements
[57]: http://creativecommons.org/licenses/by-sa/4.0/
[58]: http://creativecommons.org/licenses/by-sa/4.0/
[59]: https://arxiv.org/abs/1612.03975
[60]: https://github.com/commonsense/conceptnet5/wiki/Copying-and-sharing-ConceptNet
```
