
linux:
	gcc -O2 -fPIC -Wall -shared -o libnonlayeredtidytrees.so non-layered-tidy-trees.c

macos:
	gcc -O2 -fPIC -Wall -dynamiclib -o libnonlayeredtidytrees.dylib non-layered-tidy-trees.c

mingw:
	gcc -O2 -fPIC -Wall -shared -o libnonlayeredtidytrees.dll non-layered-tidy-trees.c

install:
	cp non-layered-tidy-trees.h /usr/local/include/
	mv libnonlayeredtidytrees.so /usr/local/lib/

install-macos:
	cp non-layered-tidy-trees.h /usr/local/include/
	mv libnonlayeredtidytrees.dylib /usr/local/lib/

mingw-install:
	cp non-layered-tidy-trees.h /usr/local/include/
	mv libnonlayeredtidytrees.dll /usr/local/lib/
