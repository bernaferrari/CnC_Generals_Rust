#ifndef CRCSTRAW_H
#define CRCSTRAW_H

#include	"crc.h"
#include	"straw.h"

/*
**	This class will build a CRC value from the data stream that is drawn through this class.
**	The data is not modified, but it is examined as it passes through.
*/
class CRCStraw : public Straw
{
	public:
		CRCStraw(void) {}
		virtual int Get(void * source, int slen);

		// Calculate and return the CRC value.
		long Result(void) const;

	protected:
		CRCEngine CRC;

	private:
		CRCStraw(CRCStraw & rvalue);
		CRCStraw & operator = (CRCStraw const & pipe);
};


#endif
