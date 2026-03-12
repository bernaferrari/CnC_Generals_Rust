#ifndef UTIL_H
#define UTIL_H

#ifndef ALWAYS_H
#include "always.h"
#endif

#include <Max.h>

#include "skin.h"
#include "nodelist.h"


/*
** Gets rid of very small numbers in the matrix
** (sets them to zero).
*/
Matrix3	Cleanup_Orthogonal_Matrix(Matrix3 & mat);

/*
** Naming utility functions
*/
void Set_W3D_Name(char * set_name,const char * src);
void Split_Node_Name(const char * name,char * set_base,char * set_exten,int * set_exten_index);
bool Append_Lod_Character(char *meshname, int lod_level, INodeListClass *origin_list);

/*
** File path utility functions
*/
void Create_Full_Path(char *full_path, const char *cwd, const char *rel_path);
void Create_Relative_Path(char *rel_path,	const char *cwd, const char *full_path);
bool Is_Full_Path(char * path);

/*
** Check if this is some kind of triangle mesh inside of MAX.  We will assume
** different default export behavior depending on whether the object is a
** triangle mesh or not.
*/
bool Is_Max_Tri_Mesh(INode * node);


/*
** Origin support.
*/
bool	Is_Origin(INode *node);
bool	Is_Base_Origin(INode *node);
INode *Find_Origin(INode *node);

bool Is_Damage_Root(INode *node);

/*
** Lod-Level and Damage State settings for an INode
*/
int Get_Lod_Level(INode *node);
int Get_Damage_State(INode *node);

/*
**  Utility function to find a named node in a hierarchy.
*/
INode *Find_Named_Node (char *nodename, INode *root);


/*
** Macros
*/
#define SAFE_DELETE(pobject)					\
			if (pobject) {							\
				delete pobject;					\
				pobject = NULL;					\
			}											\

#define SAFE_DELETE_ARRAY(pobject)			\
			if (pobject) {							\
				delete [] pobject;				\
				pobject = NULL;					\
			}											\


#endif 