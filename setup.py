from distutils.core import setup

setup(
    name="PhotoProcess",
    version="0.1dev",
    packages=["photo_process"],
    license="MIT",
    long_description=open("README.md").read(),
    entry_points={"console_scripts": ["photo_process = photo_process:main"]},
)
