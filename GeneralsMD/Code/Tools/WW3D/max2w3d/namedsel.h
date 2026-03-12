#ifndef NAMEDSEL_H
#define NAMEDSEL_H


#include "Max.h"

/*
** This is a class for containing bit arrays for
** the named selection sets.  I stole it from
** the Edit Mesh modifier code...
** It is basically a dynamically sized array
** of bitarrays.
*/
class NamedSelSetList 
{
public:
	Tab<BitArray*>			Sets;
	Tab<TSTR*>				Names;
	
	~NamedSelSetList();

	BitArray & operator[](int i) { return *Sets[i]; }
	int Count() { return Sets.Count(); }
	
	int  Find_Set(TSTR & setname);
	void Delete_Set(int i);
	void Delete_Set(TSTR & setname);
	void Reset(void);
	void Append_Set(BitArray & nset,TSTR & setname);
	
	IOResult Load(ILoad * iload);
	IOResult Save(ISave * isave);
	IOResult Load_Set(ILoad * iload);
	
	void Set_Size(int size);
	NamedSelSetList & operator=(NamedSelSetList & from);

	enum {
		NAMED_SEL_SET_CHUNK =   0x0021,
		NAMED_SEL_BITS_CHUNK =	0x0022,
		NAMED_SEL_NAME_CHUNK =	0x0023
	};
};


#endif /*NAMEDSEL_H*/
