#if _MSC_VER >= 1000
#pragma once
#endif // _MSC_VER >= 1000

#ifndef RNDSTRNG_H
#define RNDSTRNG_H

#include	"vector.h"
#include	"random.h"

class	StringClass;

/*
**
*/
class	RandomStringClass {

public:
	// Add a string to the class.  
	// (future version may have a weight parameter)
	void Add_String( const char * str );

	// Get a random string from the class
	const char * Get_String( void );

private:
	DynamicVectorClass<StringClass>	Strings;
	Random2Class							Randomizer;
};

#endif // RNDSTRNG_H

