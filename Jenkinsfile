
try {
	node('windows-slave-2') {
		try {
			checkout scm
bat '''
make mingw
'''
		} finally {
			//cleanWs()
		}
	}
} catch(e) {
	throw e
}