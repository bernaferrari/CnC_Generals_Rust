#include "shdinterface.h"
#include "shddef.h"


//**********************************************************************************************
//! Constructor
/*!
	Base class constructor for shaders.  Saves a pointer to the definition for this shader.

	@param 
*/
ShdInterfaceClass::ShdInterfaceClass(const ShdDefClass * def, int class_id) : 
	Definition(NULL),
	ClassID(class_id)
{ 
	REF_PTR_SET(Definition,def); 
}


//**********************************************************************************************
//! Destructor
/*!
	Destructor for ShdInterfaceClass, releases the definition.
*/
ShdInterfaceClass::~ShdInterfaceClass(void) 
{ 
	REF_PTR_RELEASE(Definition); 
}


//**********************************************************************************************
//! returns a pointer to the definition for this shader
/*!
	@returns	
*/
const ShdDefClass * ShdInterfaceClass::Peek_Definition(void)
{
	return Definition;
}
