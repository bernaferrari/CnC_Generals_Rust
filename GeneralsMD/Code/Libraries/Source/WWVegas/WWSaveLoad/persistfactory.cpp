#include "persistfactory.h"
#include "saveload.h"

PersistFactoryClass::PersistFactoryClass(void) : 
	NextFactory(NULL)
{
	SaveLoadSystemClass::Register_Persist_Factory(this);
}

PersistFactoryClass::~PersistFactoryClass(void)
{
	SaveLoadSystemClass::Unregister_Persist_Factory(this);
}
