#ifndef SHDCLASSIDS_H
#define SHDCLASSIDS_H


/*
** ClassID's for Shader Definitions
*/
enum
{
	SHDDEF_CLASSID_DUMMY = 0,
	SHDDEF_CLASSID_SIMPLE,
	SHDDEF_CLASSID_GLOSSMASK,
	SHDDEF_CLASSID_BUMPSPEC,
	SHDDEF_CLASSID_BUMPDIFF,
	SHDDEF_CLASSID_CUBEMAP,
	SHDDEF_CLASSID_LEGACYW3D,
	SHDDEF_CLASSID_LAST,
};


/*
** ClassID's for actual Shader Implementations (typically there will be several for each "type", one
** for each hardware configuration...)
*/
enum 
{
	SHD_CLASSID_DUMMY = 0,
	SHD_CLASSID_LAST,
};


#endif //SHDCLASSIDS_H
