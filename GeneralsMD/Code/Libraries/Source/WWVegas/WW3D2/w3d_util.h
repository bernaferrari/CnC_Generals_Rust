#if defined(_MSC_VER)
#pragma once
#endif

#ifndef W3D_UTIL_H
#define W3D_UTIL_H

#include "always.h"
#include "w3d_file.h"

class Vector3;
class Vector4;
class Quaternion;
class ShaderClass;

/*
** These functions are just simple helpers which convert to and from the W3D file-format
** structures and the equivalent runtime classes.
*/
class W3dUtilityClass
{
public:

	static void	Convert_Vector(const W3dVectorStruct & v,Vector3 * set);
	static void	Convert_Vector(const Vector3 & v,W3dVectorStruct * set);

	static void	Convert_Quaternion(const W3dQuaternionStruct & q,Quaternion * set);
	static void	Convert_Quaternion(const Quaternion & q,W3dQuaternionStruct * set);

	static void	Convert_Color(const W3dRGBStruct & rgb,Vector3 * set);
	static void	Convert_Color(const Vector3 & v,W3dRGBStruct * set);

	static void	Convert_Color(const W3dRGBAStruct & rgb,Vector4 * set);
	static void	Convert_Color(const Vector4 & v,W3dRGBAStruct * set);
	
	static void Convert_Shader(const W3dShaderStruct & shader,ShaderClass * set);
	static void Convert_Shader(const ShaderClass & shader,W3dShaderStruct * set);

};

#endif


