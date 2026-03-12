#ifndef SHDDEFFACTORY_H
#define SHDDEFFACTORY_H

#include "always.h"
#include "bittype.h"

class ShdDefClass;

/*
** NOTE:  For most users, the only thing you need from this module is the REGISTER_SHDDEF(T,ID,NAME) macro
*/


/**
** ShdDefFactoryClass - An instance of this class is used to automatically register
** each unique type of ShdDefClass with the system.  This object is responsible for
** creating shader definitions.  All existing 'DefFactories' can be iterated over 
** and presented to the user in a menu.
*/
class ShdDefFactoryClass 
{
public:
	ShdDefFactoryClass (void);
	virtual ~ShdDefFactoryClass (void);

	//////////////////////////////////////////////////////////////
	//	Public methods
	//////////////////////////////////////////////////////////////
	virtual ShdDefClass *			Create (void) const = 0;
	virtual const char *				Get_Name (void) const = 0;
	virtual uint32						Get_Class_ID (void) const = 0;

protected:

	//////////////////////////////////////////////////////////////
	//	Protected member data
	//////////////////////////////////////////////////////////////
	ShdDefFactoryClass *				NextFactory;
	ShdDefFactoryClass *				PrevFactory;

	friend class ShdDefManagerClass;
};


/**
** SimpleShdDefFactoryClass - This template automates the process of creating a ShdDefFactory. 
** For complete ease of use, the associated REGISTER_SHDDEF macro can be used in the cpp file
** of your shader definition.
** Macro useage example (in the cpp file for your shader definition):
** REGISTER_SHDDEF(MyShdDefClass, MYSHDDEF_CLASSID, "Greg's super-fancy shader");
*/
template<class T, int class_id, char *name> class SimpleShdDefFactoryClass : public ShdDefFactoryClass
{
public:
	SimpleShdDefFactoryClass (void) { }
	virtual ~SimpleShdDefFactoryClass (void) { }

	//////////////////////////////////////////////////////////////
	//	Public methods
	//////////////////////////////////////////////////////////////
	virtual ShdDefClass *		Create (void) const					{ return new T; }
	virtual const char *			Get_Name (void) const				{ return name; }
	virtual uint32					Get_Class_ID (void) const			{ return class_id; }
};


/*
** Use this macro in the .CPP file for your shader definition to automatically link it
** into the shader editing system.
*/
#define REGISTER_SHDDEF(T,ID,NAME)							\
char T ## Name[] = NAME;										\
SimpleShdDefFactoryClass<T,ID,T ## Name> T ## Factory \



#endif //SHDDEFFACTORY_H
