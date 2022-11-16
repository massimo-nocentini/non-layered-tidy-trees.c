
linux:
	gcc -O2 -std=gnu99 -fPIC -Wall -I/usr/local/include/ -s -shared -o libnonlayeredtidytrees.so non-layered-tidy-trees.c

install:
	cp timsort.h /usr/local/include/
	mv libtimsort.so /usr/local/lib/
