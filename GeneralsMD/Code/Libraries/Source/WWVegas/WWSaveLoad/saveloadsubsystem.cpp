#include "saveloadsubsystem.h"
#include "saveload.h"


SaveLoadSubSystemClass::SaveLoadSubSystemClass(void) :
	NextSubSystem(NULL)
{
	// All Sub-Systems are automatically registered with the SaveLoadSystem
	SaveLoadSystemClass::Register_Sub_System (this);
	return ;
}

SaveLoadSubSystemClass::~SaveLoadSubSystemClass (void)
{
	SaveLoadSystemClass::Unregister_Sub_System (this);
	return ;
}
