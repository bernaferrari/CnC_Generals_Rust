#include "textfile.h"
#include "wwstring.h"


///////////////////////////////////////////////////////////////////////////////
//
//	TextFileClass
//
///////////////////////////////////////////////////////////////////////////////
TextFileClass::TextFileClass (void)
{
	return ;
}


///////////////////////////////////////////////////////////////////////////////
//
//	TextFileClass
//
///////////////////////////////////////////////////////////////////////////////
TextFileClass::TextFileClass (char const *filename)
	:	RawFileClass (filename)
{
	return ;
}


///////////////////////////////////////////////////////////////////////////////
//
//	~TextFileClass
//
///////////////////////////////////////////////////////////////////////////////
TextFileClass::~TextFileClass (void)
{
	return ;
}


///////////////////////////////////////////////////////////////////////////////
//
//	Read_Line
//
///////////////////////////////////////////////////////////////////////////////
bool
TextFileClass::Read_Line (StringClass &string)
{
	//
	//	Start with a fresh string
	//
//	string.Empty ();
	string="";

	const int BUFFER_SIZE		= 64;
	char buffer[BUFFER_SIZE]	= { 0 };
	bool keep_going				= true;

	while (keep_going) {

		//
		// Read a chunk of characters from the file
		//
		int size = Read (buffer, BUFFER_SIZE - 1);		

		//
		// Keep going if we still have more data to 
		// read from the file
		//
		keep_going = (size == BUFFER_SIZE - 1);
		if (size > 0) {			
			
			//
			// Try to find the linefeed character
			//
			for (int index = 0; index < size; index ++) {
				if (buffer[index] == '\n') {
					
					//
					// Terminate the buffer after the linefeed
					//
					buffer[index + 1] = 0;

					//
					// Seek backwards in the file to the position
					// directly after the linefeed
					//
					Seek (-(size - (index + 1)), SEEK_CUR);
					keep_going = false;
					break;
				}
			}
			
			//
			//	Concat this buffer to the end of the string
			//
			string += buffer;
		}
	}

	bool retval = (string.Get_Length () > 0);
	if (retval) {
		
		int len				= string.Get_Length ();
		char *raw_string	= string.Peek_Buffer ();
		
		//
		// Strip the CR\LF or LF from the string
		//		
		if (len > 1 && raw_string[len - 2] == '\r') {
			string.Erase (len - 2, 2);
			//raw_string[len - 2] = 0;
		} else if (raw_string[len - 1] == '\n') {
			string.Erase (len - 1, 1);
			//raw_string[len - 1] = 0;
		}
	}
	
	return retval;
}


///////////////////////////////////////////////////////////////////////////////
//
//	Write_Line
//
///////////////////////////////////////////////////////////////////////////////
bool
TextFileClass::Write_Line (const StringClass &string)
{
	bool retval = false;

	//
	// Write the line of text out to the file
	//
	int len = string.Get_Length ();
	int size = Write ((const char *)string, len);

	//
	// Now append a CR\LF pair to the end of the line
	//
	if (size == len) {
		retval = (Write ("\r\n", 2) == 2);
	}

	return retval;
}
