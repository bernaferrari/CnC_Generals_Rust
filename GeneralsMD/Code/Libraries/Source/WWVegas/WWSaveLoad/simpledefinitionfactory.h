

#ifndef __SIMPLE_DEFINITION_FACTORY_H
#define __SIMPLE_DEFINITION_FACTORY_H

#include "definitionfactory.h"

//////////////////////////////////////////////////////////////////////////////////
//
//	SimpleDefinitionFactoryClass 
//
//	Template class to automate the creation of simple definition factories.
//
//////////////////////////////////////////////////////////////////////////////////
template<class T, int class_id, char *name>
class SimpleDefinitionFactoryClass : public DefinitionFactoryClass
{
public:

	//////////////////////////////////////////////////////////////
	//	Public constructors/destructors
	//////////////////////////////////////////////////////////////
	SimpleDefinitionFactoryClass (bool is_displayed = true)	:
		IsDisplayed (is_displayed)											{}

	//////////////////////////////////////////////////////////////
	//	Public methods
	//////////////////////////////////////////////////////////////
	virtual DefinitionClass *	Create (void) const;
	virtual const char *			Get_Name (void) const;
	virtual uint32					Get_Class_ID (void) const;
	virtual bool					Is_Displayed (void) const	{ return IsDisplayed; }

protected:

	//////////////////////////////////////////////////////////////
	//	Protected member data
	//////////////////////////////////////////////////////////////
	bool		IsDisplayed;
};

template<class T, int class_id, char *name>
inline DefinitionClass *
SimpleDefinitionFactoryClass<T, class_id, name>::Create (void) const
{
	return W3DNEW T;
}

template<class T, int class_id, char *name>
inline const char *
SimpleDefinitionFactoryClass<T, class_id, name>::Get_Name (void) const
{
	return name;
}

template<class T, int class_id, char *name>
inline uint32
SimpleDefinitionFactoryClass<T, class_id, name>::Get_Class_ID (void) const
{
	return class_id;
}

#define DECLARE_DEFINITION_FACTORY(_class, _id, _name)		\
char _class ## Name[] = _name;										\
SimpleDefinitionFactoryClass<_class, _id, _class ## Name>	\

#endif //__SIMPLE_DEFINITION_FACTORY_H

