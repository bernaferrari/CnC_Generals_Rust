#ifndef SHDDEFMANAGER_H
#define SHDDEFMANAGER_H

#include "always.h"
#include "bittype.h"

class ShdDefClass;
class ShdDefFactoryClass;
class ChunkSaveClass;
class ChunkLoadClass;


/**
** ShdDefManagerClass - This class contains a list of all constructed ShdDefFactories.  
** This class is used to iterate through the shader definition factories in the material
** editor for example.  
*/
class ShdDefManagerClass
{
public:

	static ShdDefFactoryClass *		Find_Factory (uint32 class_id);	
	static ShdDefFactoryClass *		Find_Factory (const char *name);
	static void								Register_Factory (ShdDefFactoryClass *factory);
	static void								Unregister_Factory (ShdDefFactoryClass *factory);

	// Class enumeration
	static ShdDefFactoryClass *		Get_First (uint32 superclass_id);
	static ShdDefFactoryClass *		Get_Next (ShdDefFactoryClass *current, uint32 superclass_id);

	// Factory enumeration
	static ShdDefFactoryClass *		Get_First (void);
	static ShdDefFactoryClass *		Get_Next (ShdDefFactoryClass *current);
	
	// Construction
	static ShdDefClass *					Create_ShdDefClass_Instance(uint32 class_id);

	// save/load
	static void								Save_Shader(ChunkSaveClass& csave, ShdDefClass* shddef);
	static void								Load_Shader(ChunkLoadClass& cload, ShdDefClass** shddef);

private:

	static void								Link_Factory (ShdDefFactoryClass *factory);
	static void								Unlink_Factory (ShdDefFactoryClass *factory);

	static ShdDefFactoryClass *		_FactoryListHead;
};



#endif SHDDEFMANAGER_H