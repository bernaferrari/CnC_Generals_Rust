#include	"rndstrng.h"
#include	"wwstring.h"

/*
**
*/

void RandomStringClass::Add_String( const char * str )
{
	Strings.Add( str );
}

const char * RandomStringClass::Get_String( void )
{
	if ( Strings.Count() == 0 ) {
		return NULL;
	}
	unsigned int index = Randomizer();
	index %= Strings.Count();
	return Strings[index];
}

