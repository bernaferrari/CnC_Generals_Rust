#include "shddeffactory.h"
#include "shddefmanager.h"


/*
**
** ShdDefFactory Implementation
**
*/

//**********************************************************************************************
//! Constructor for ShdDefFactoryClass
/*!
	Automatically registers this instance of the ShdDefFactory with the manager.
*/
ShdDefFactoryClass::ShdDefFactoryClass (void) : 
	NextFactory(NULL), 
	PrevFactory(NULL) 
{ 
	ShdDefManagerClass::Register_Factory (this);
}


//**********************************************************************************************
//! Destructor for ShdDefFactoryClass
/*!
	Automatically un-registers this instance of a ShdDefFactory from the manager.
*/
ShdDefFactoryClass::~ShdDefFactoryClass (void) 
{
	ShdDefManagerClass::Unregister_Factory (this);
}



