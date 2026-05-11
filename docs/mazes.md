# Mazes

- [Levels](#levels)
- [Links](#links)

## Levels

In contrast to the spec, I chose to rank my mazes in terms of actual ease of navigation rather than tendency for dead ends; the two are often at odds:

| **Level** | **Navigational ease** | **Dead-end density** |
| --- | --- | --- |
| 0 | Four-Quadrants Binary Tree | Standard Recursive Division (~11%) |
| 1 | Standard Recursive Division | Backtracker (~13%) |
| 2 | Meander[^1] | Territorial Recursive Division (~15%) |
| 3 | Territorial Recursive Division | Hecate's Key (~20%) |
| 4 | Hecate's Key | Wilson (~28%) |
| 5 | Prim | Kruskal (~38%) |
| 6 | Kruskal | Prim (~48%) |
| 7 | Drunkard's Walk | Drunkard's Walk (~50%) |
| 8 | Backtracker | Four-Quadrants Binary Tree (50% Fixed) |
| 9 | Wilson | Meander (50%+) |

My Territorial Recursive Division is Jamis Buck's [Better Recursive Division Algorithm](https://weblog.jamisbuck.org/2015/1/15/better-recursive-division-algorithm.html).

Percentages from Gemini, so take them with a pinch of salt. I haven't found a proof or experimental evidence for all of them yet. Gemini vacilates over whether recursive division or randomized backtracker has fewest dead ends, but the rankings don't shuffle wildly between responses. Its figures are roughly consistent with those that I have found, e.g. Mane et al. report DFS (i.e. Backtracker): 10.0, Wilson: 30.0, Kruskal: 30.6, Prim: 35.5.[^2] Their ranking of these algorithms in terms of difficulty also matches Gemini's.

## Links

- Jamis Buck: [The Buckblog](https://weblog.jamisbuck.org/archives.html).
- Jamis Buck: [Mazes for Programmers](http://www.mazesforprogrammers.com/)
- Wikipedia: [Maze generation algorithm](https://en.wikipedia.org/wiki/Maze_generation_algorithm)

[^1]: Some of these algorithms use different names internally: Meander is `TwiggyDividerQueue`, Hecate's Key is `BlobbyDividerQueue`, Drunkard's Walk is `TwiggyDividerRandom`, and Territorial Recursive Division is `BlobbyDividerRandom`.

[^2]: Deepak Mane, Rajat Harne, Tanmay Pol, Rashmi Asthagi, Sandip Shine, Bhushan Zope: [An Extensive Comparative Analysis on Different MazeGeneration Algorithms](https://ijisae.org/index.php/IJISAE/article/view/3557). International Journal of Intelligent Systems and Applications in Engineering. IJISAE, 2024, 12(2s), 37–47
