#if _MSC_VER >= 1000
#pragma once
#endif // _MSC_VER >= 1000

#ifndef BUFFFILE_H
#define BUFFFILE_H

#include	"rawfile.h"


/*
**	This is the definition of a buffered read raw file class. 
*/
class BufferedFileClass : public RawFileClass
{
	typedef RawFileClass BASECLASS;

	public:

		BufferedFileClass(char const *filename);
		BufferedFileClass(void);
		BufferedFileClass (RawFileClass const & f);
		BufferedFileClass & operator = (BufferedFileClass const & f);
		virtual ~BufferedFileClass(void);

		virtual int Read(void *buffer, int size);
		virtual int Seek(int pos, int dir=SEEK_CUR);
		virtual int Write(void const *buffer, int size);
		virtual void Close(void);

	protected:

		static	void		Set_Desired_Buffer_Size( int size ) { _DesiredBufferSize = size; }

		void					Reset_Buffer( void );
		
	private:
		unsigned char *	Buffer;				// The read buffer 
		unsigned int		BufferSize;			// The allocated size of the read buffer
		int					BufferAvailable;	// The amount of data in the read buffer
		int					BufferOffset;		// The data already given out
		static	int		_DesiredBufferSize;
};

#endif
