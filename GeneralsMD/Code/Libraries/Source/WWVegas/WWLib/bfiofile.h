#ifndef BFIOFILE_H
#define BFIOFILE_H

#include	"rawfile.h"

/*
**	This derivation of the raw file class handles buffering the input/output in order to
**	achieve greater speed. The buffering is not active by default. It must be activated
**	by setting the appropriate buffer through the Cache() function.
*/
class BufferIOFileClass : public RawFileClass
{
		typedef RawFileClass BASECLASS;

	public:

		BufferIOFileClass(char const * filename);
		BufferIOFileClass(void);
		virtual ~BufferIOFileClass(void);

		bool Cache( long size=0, void * ptr=NULL);
		void Free( void);
		bool Commit( void);
		virtual char const * Set_Name(char const * filename);
		virtual bool Is_Available(int forced=false);
		virtual bool Is_Open(void) const;
		virtual int Open(char const * filename, int rights=READ);
		virtual int Open(int rights=READ);
		virtual int Read(void * buffer, int size);
		virtual int Seek(int pos, int dir=SEEK_CUR);
		virtual int Size(void);
		virtual int Write(void const * buffer, int size);
		virtual void Close(void);

		enum {MINIMUM_BUFFER_SIZE=1024};

	private:

		bool IsAllocated;
		bool IsOpen;
		bool IsDiskOpen;
		bool IsCached;
		bool IsChanged;
		bool UseBuffer;

		int BufferRights;

		void *Buffer;

		long BufferSize;
		long BufferPos;
		long BufferFilePos;
		long BufferChangeBeg;
		long BufferChangeEnd;
		long FileSize;
		long FilePos;
		long TrueFileStart;
};

#endif
