
linux:
	gcc -O2 -fPIC -Wall -s -shared -o libnonlayeredtidytrees.so non-layered-tidy-trees.c

mingw:
	gcc -O2 -fPIC -Wall -s -shared -o libnonlayeredtidytrees.dll non-layered-tidy-trees.c

install:
	cp non-layered-tidy-trees.h /usr/local/include/
	mv libnonlayeredtidytrees.so /usr/local/lib/

mingw-install:
	cp non-layered-tidy-trees.h /usr/local/include/
	mv libnonlayeredtidytrees.dll /usr/local/lib/
