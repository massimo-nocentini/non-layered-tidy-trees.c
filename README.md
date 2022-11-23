# non-layered-tidy-trees.c

This repository contains a translation in C of the algorithm described in the publication

>van der Ploeg, A. (2014), Drawing non-layered tidy trees in linear time, Softw. Pract. Exper., 44, pages 1467– 1484, doi: [10.1002/spe.2213](https://doi.org/10.1002/spe.2213)

which is implemented in the companion https://github.com/cwi-swat/non-layered-tidy-trees repository. The code is almost identical but the necessary technicalities to let `gcc` compile the project; by the way, we have enhanced it providing both *horizontal layouting*, *centered `x`s and `y`s* and *callbacks*. The result is a shared library that can be linked in arbitrary projects.

According to the issue https://github.com/Klortho/d3-flextree/issues/1, we take the same fix as done in https://github.com/Klortho/d3-flextree/blob/af196220927218bbe7ac6cad8e059f56430befb6/src/flextree.js#L242
