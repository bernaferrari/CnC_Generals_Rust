#include "definitionfactory.h"
#include "definitionfactorymgr.h"


/////////////////////////////////////////////////////////
//
//	DefinitionFactoryClass
//
/////////////////////////////////////////////////////////
DefinitionFactoryClass::DefinitionFactoryClass (void)
	:	m_NextFactory (0),
		m_PrevFactory (0)
{
	DefinitionFactoryMgrClass::Register_Factory (this);
	return ;
}


/////////////////////////////////////////////////////////
//
//	~DefinitionFactoryClass
//
/////////////////////////////////////////////////////////
DefinitionFactoryClass::~DefinitionFactoryClass (void)
{
	DefinitionFactoryMgrClass::Unregister_Factory (this);
	return ;
}
