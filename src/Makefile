
linux:
	gcc -O2 -std=gnu99 -fPIC -Wall -s -shared -o libnonlayeredtidytrees.so non-layered-tidy-trees.c

install:
	cp non-layered-tidy-trees.h /usr/local/include/
	mv libnonlayeredtidytrees.so /usr/local/lib/
