#ifndef SHASTRAW_H
#define SHASTRAW_H


#include	"sha.h"
#include	"straw.h"

/*
**	This class serves as a straw that generates a Secure Hash from the data stream that flows
**	through it. It doesn't modify the data stream in any fashion.
*/
class SHAStraw : public Straw
{
	public:
		SHAStraw(void) : IsDisabled(false) {}
		virtual int Get(void * source, int slen);

		void Disable(void) {IsDisabled = true;}
		void Enable(void) {IsDisabled = false;}

		// Fetch the SHA hash value (stored in result buffer -- 20 bytes long).
		int Result(void * result) const;

	protected:
		bool IsDisabled;

		SHAEngine SHA;

	private:
		SHAStraw(SHAStraw & rvalue);
		SHAStraw & operator = (SHAStraw const & straw);
};


#endif
