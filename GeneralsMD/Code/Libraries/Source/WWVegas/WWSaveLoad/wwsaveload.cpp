#include "wwsaveload.h"
#include "definitionmgr.h"


void WWSaveLoad::Init(void)
{
}

void WWSaveLoad::Shutdown(void)
{
	_TheDefinitionMgr.Free_Definitions();
}


