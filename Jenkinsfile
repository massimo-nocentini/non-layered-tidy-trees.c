
try {
	node('windows-slave-2') {
		try {
			checkout scm
bat '''
cd src
make mingw
cd ..
zip -r libnonlayeredtidytrees.zip src/libnonlayeredtidytrees.dll src/non-layered-tidy-trees.h	

'''
		} finally {
			archiveArtifacts artifacts: 'libnonlayeredtidytrees.zip'
			cleanWs()
		}
	}
} catch(e) {
	throw e
}