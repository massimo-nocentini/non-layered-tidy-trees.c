
try {
	node('windows-slave-2') {
		try {
			checkout scm
bat '''
cd src
make mingw
zip -r libnonlayeredtidytrees.zip libnonlayeredtidytrees.dll non-layered-tidy-trees.h	

'''
		} finally {
			archiveArtifacts artifacts: 'src/libnonlayeredtidytrees.zip'
			cleanWs()
		}
	}
} catch(e) {
	throw e
}