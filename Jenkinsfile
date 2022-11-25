
try {
	node('windows-slave-2') {
		try {
			checkout scm
bat '''
cd src
make mingw
'''
		} finally {
			//cleanWs()
		}
	}
} catch(e) {
	throw e
}