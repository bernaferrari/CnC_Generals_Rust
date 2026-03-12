#ifndef CRCPIPE_H
#define CRCPIPE_H

#include	"crc.h"
#include	"pipe.h"

/*
**	This class doesn't modify the data being piped through, but it does examine it and build
**	a CRC value from the data.
*/
class CRCPipe : public Pipe
{
	public:
		CRCPipe(void) {}
		virtual int Put(void const * source, int slen);

		// Fetch the CRC value.
		long Result(void) const;

	protected:
		CRCEngine CRC;

	private:
		CRCPipe(CRCPipe & rvalue);
		CRCPipe & operator = (CRCPipe const & pipe);
};

#endif
