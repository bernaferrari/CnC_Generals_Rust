// FILE: INIException.h ///////////////////////////////////////////////////////////////////////////
// Author: John McDonald, Jr, October 2002
// Desc:   INI Exception class. Thrown when INIs fail to read.
///////////////////////////////////////////////////////////////////////////////////////////////////

class INIException
{
	// This is a stack based exception class. It is used to output useful information
	// when thrown from an INI message

public:
	char *mFailureMessage;

	INIException(const char* errorMessage) : mFailureMessage(NULL)
	{
		if (errorMessage) {
			mFailureMessage = new char[strlen(errorMessage) + 1];
			strcpy(mFailureMessage, errorMessage);
		}
	}

	~INIException()
	{
		if (mFailureMessage) {
			delete [] mFailureMessage;
		}
	}
};