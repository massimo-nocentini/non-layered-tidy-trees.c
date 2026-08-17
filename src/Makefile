
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
	mkdir -p /usr/local/include	# just for ensuring that the dest dir exists
	mkdir -p /usr/local/lib	# just for ensuring that the dest dir exists
	cp non-layered-tidy-trees.h /usr/local/include/
	mv libnonlayeredtidytrees.dll /usr/local/lib/

# Without an explicit export list there is no `main` to root the call graph, so
# `-O2` strips the whole library and the resulting module exports nothing usable.
EMCC_EXPORTS = _layout_api,_malloc,_free
EMCC_RUNTIME_EXPORTS = ccall,cwrap,HEAPF64,HEAP32

emscripten:
	docker run --rm -v $$(pwd):/src -u $$(id -u):$$(id -g) emscripten/emsdk:latest emcc -O2 -sWASM_BIGINT -sEXPORTED_FUNCTIONS=$(EMCC_EXPORTS) -sEXPORTED_RUNTIME_METHODS=$(EMCC_RUNTIME_EXPORTS) non-layered-tidy-trees.c -o non-layered-tidy-trees.js
